use std::{
    net::SocketAddr,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use quic_tunnel_control_client::{
    HttpControlClient, RegisterRelayRequest, RelayBootstrapExchangeRequest,
    RelayBootstrapExchangeResponse, RelayCommand, RelayCommandKind, RelayCommandStatus,
    RelayHealthReport, RelayHealthStatus, RelaySessionSnapshot, RelaySessionUsageReport,
    ReportRelayCommandResultRequest, ReportRelayHealthRequest, ReportRelaySessionUsageRequest,
};
use quic_tunnel_relay::{
    config::RelayConfig,
    runtime::RelayService,
    session::{RelaySession, RelaySessionState, RelaySessionStore},
};
use tokio::sync::watch;

#[derive(Debug, Parser)]
#[command(name = "relayd")]
#[command(about = "Standalone Relay service for MobileCode Connect")]
struct Cli {
    #[arg(long, default_value = "127.0.0.1:4443")]
    bind: SocketAddr,
    #[arg(long, env = "QUIC_TUNNEL_RELAY_TOKEN_SECRET")]
    token_secret: Option<String>,
    #[arg(long)]
    now_epoch_sec: Option<u64>,
    #[arg(long)]
    debug_admin_listen: Option<SocketAddr>,
    #[arg(long, hide = true)]
    admin_listen: Option<SocketAddr>,
    #[arg(long)]
    cert_out: Option<PathBuf>,
    #[arg(long)]
    control_url: Option<String>,
    #[arg(long)]
    control_token: Option<String>,
    #[arg(long)]
    relay_id: Option<String>,
    #[arg(long)]
    advertise_addr: Option<String>,
    #[arg(long, hide = true)]
    advertise_admin_addr: Option<String>,
    #[arg(long, default_value_t = 128)]
    capacity_streams: u32,
    #[arg(long, default_value_t = 30)]
    heartbeat_interval_sec: u64,
    #[arg(long)]
    bootstrap_control_url: Option<String>,
    #[arg(long)]
    bootstrap_id: Option<String>,
    #[arg(long)]
    bootstrap_token: Option<String>,
}

impl Cli {
    fn config(&self, bootstrap: Option<&RelayBootstrapExchangeResponse>) -> Result<RelayConfig> {
        let token_secret = bootstrap
            .map(|bootstrap| bootstrap.token_secret.clone())
            .or_else(|| self.token_secret.clone())
            .context("--token-secret is required unless relayd is started with bootstrap args")?;
        Ok(RelayConfig {
            token_secret,
            now_epoch_sec: self.now_epoch_sec.unwrap_or_else(current_epoch_sec),
        })
    }

    fn heartbeat_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.heartbeat_interval_sec.max(1))
    }

    fn debug_admin_listen(&self) -> Option<SocketAddr> {
        self.debug_admin_listen.or(self.admin_listen)
    }

    fn control_registration(
        &self,
        relay_addr: SocketAddr,
    ) -> Result<Option<RelayControlRegistration>> {
        if self.bootstrap_exchange()?.is_some() {
            return Ok(None);
        }
        let Some(control_url) = self
            .control_url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            return Ok(None);
        };
        let Some(control_token) = self
            .control_token
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            bail!("--control-token is required when --control-url is set");
        };
        let Some(relay_id) = self
            .relay_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            bail!("--relay-id is required when --control-url is set");
        };
        if self.capacity_streams == 0 {
            bail!("--capacity-streams must be greater than zero");
        }

        let relay_addr = self
            .advertise_addr
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| relay_addr.to_string());
        let _legacy_advertise_admin_addr = self.advertise_admin_addr.as_deref();
        let admin_addr = String::new();

        Ok(Some(RelayControlRegistration {
            control_url: control_url.to_string(),
            control_token: control_token.to_string(),
            relay_id: relay_id.to_string(),
            relay_addr,
            admin_addr,
            capacity_streams: self.capacity_streams,
        }))
    }

    fn bootstrap_exchange(&self) -> Result<Option<RelayBootstrapExchange>> {
        match (
            self.bootstrap_control_url.as_deref(),
            self.bootstrap_id.as_deref(),
            self.bootstrap_token.as_deref(),
        ) {
            (None, None, None) => Ok(None),
            (Some(control_url), Some(bootstrap_id), Some(bootstrap_token)) => {
                if self.control_url.is_some()
                    || self.control_token.is_some()
                    || self.relay_id.is_some()
                    || self.token_secret.is_some()
                {
                    bail!("bootstrap args cannot be combined with explicit control registration or --token-secret");
                }
                let control_url = control_url.trim();
                let bootstrap_id = bootstrap_id.trim();
                let bootstrap_token = bootstrap_token.trim();
                if control_url.is_empty() || bootstrap_id.is_empty() || bootstrap_token.is_empty() {
                    bail!("bootstrap args must not be empty");
                }
                Ok(Some(RelayBootstrapExchange {
                    control_url: control_url.to_string(),
                    bootstrap_id: bootstrap_id.to_string(),
                    bootstrap_token: bootstrap_token.to_string(),
                }))
            }
            _ => bail!(
                "--bootstrap-control-url, --bootstrap-id, and --bootstrap-token must be provided together"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RelayControlRegistration {
    control_url: String,
    control_token: String,
    relay_id: String,
    relay_addr: String,
    admin_addr: String,
    capacity_streams: u32,
}

impl RelayControlRegistration {
    fn from_bootstrap(response: RelayBootstrapExchangeResponse) -> Self {
        Self {
            control_url: response.control_url,
            control_token: response.control_token,
            relay_id: response.relay_id,
            relay_addr: response.relay_addr,
            admin_addr: String::new(),
            capacity_streams: response.capacity_streams,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RelayBootstrapExchange {
    control_url: String,
    bootstrap_id: String,
    bootstrap_token: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    let bootstrap = if let Some(exchange) = cli.bootstrap_exchange()? {
        Some(exchange_relay_bootstrap_with_control(exchange).await?)
    } else {
        None
    };
    let service = RelayService::new_quic(cli.config(bootstrap.as_ref())?, cli.bind)
        .await
        .context("start relay quic endpoint")?;
    let started_at = std::time::Instant::now();

    if let Some(cert_out) = &cli.cert_out {
        let cert = service
            .certificate_der()
            .context("relay service did not create a certificate")?;
        tokio::fs::write(cert_out, cert.as_ref())
            .await
            .with_context(|| format!("write relay certificate to {}", cert_out.display()))?;
    }

    let addr = service
        .local_addr()
        .context("relay service did not expose a local address")?;
    let admin_listener = if let Some(admin_listen) = cli.debug_admin_listen() {
        Some(
            tokio::net::TcpListener::bind(admin_listen)
                .await
                .with_context(|| format!("bind relay admin listener on {admin_listen}"))?,
        )
    } else {
        None
    };
    let admin_addr = admin_listener
        .as_ref()
        .map(|listener| listener.local_addr())
        .transpose()
        .context("relay admin listener did not expose a local address")?;
    let admin_routes = admin_listener.as_ref().map(|_| service.admin_routes());
    let data_plane_bound = true;
    let admin_bound = admin_listener.is_some();

    let heartbeat_registration = if let Some(bootstrap) = bootstrap {
        let registration = RelayControlRegistration::from_bootstrap(bootstrap);
        let control_url = registration.control_url.clone();
        let relay_id = registration.relay_id.clone();
        register_with_control(registration.clone())
            .await
            .with_context(|| format!("register relay {relay_id} with control {control_url}"))?;
        println!("relayd registered {relay_id} with control {control_url}");
        Some(registration)
    } else if let Some(registration) = cli.control_registration(addr)? {
        let control_url = registration.control_url.clone();
        let relay_id = registration.relay_id.clone();
        register_with_control(registration.clone())
            .await
            .with_context(|| format!("register relay {relay_id} with control {control_url}"))?;
        println!("relayd registered {relay_id} with control {control_url}");
        Some(registration)
    } else {
        None
    };

    println!("relayd listening on {addr}");
    let relay_session_store = service.session_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let relay_task = tokio::spawn(service.run_until(wait_for_shutdown(shutdown_rx)));
    let heartbeat_task = heartbeat_registration.map(|registration| {
        let heartbeat_shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(run_control_heartbeat_until(
            registration,
            cli.heartbeat_interval(),
            relay_session_store,
            data_plane_bound,
            admin_bound,
            started_at,
            heartbeat_shutdown_rx,
        ))
    });

    let admin_task = if let (Some(listener), Some(routes)) = (admin_listener, admin_routes) {
        println!("relayd admin listening on {}", admin_addr.unwrap());
        let admin_shutdown_rx = shutdown_tx.subscribe();
        Some(tokio::spawn(async move {
            axum::serve(listener, routes)
                .with_graceful_shutdown(wait_for_shutdown(admin_shutdown_rx))
                .await
        }))
    } else {
        None
    };

    tokio::signal::ctrl_c().await.context("wait for ctrl-c")?;
    let _ = shutdown_tx.send(true);

    relay_task.await.context("relay task join")??;
    if let Some(task) = admin_task {
        task.await.context("relay admin task join")??;
    }
    if let Some(task) = heartbeat_task {
        task.await.context("relay heartbeat task join")?;
    }
    Ok(())
}

async fn register_with_control(registration: RelayControlRegistration) -> Result<()> {
    let client = HttpControlClient::with_bearer_token(
        registration.control_url.clone(),
        registration.control_token.clone(),
    )?;
    client
        .register_relay(RegisterRelayRequest {
            relay_id: registration.relay_id,
            relay_addr: registration.relay_addr,
            admin_addr: registration.admin_addr,
            capacity_streams: registration.capacity_streams,
        })
        .await?;
    Ok(())
}

async fn exchange_relay_bootstrap_with_control(
    exchange: RelayBootstrapExchange,
) -> Result<RelayBootstrapExchangeResponse> {
    let client = HttpControlClient::new(exchange.control_url.clone())?;
    client
        .exchange_relay_bootstrap(
            &exchange.bootstrap_id,
            RelayBootstrapExchangeRequest {
                bootstrap_token: exchange.bootstrap_token,
            },
        )
        .await
        .context("exchange relay bootstrap with control")
}

async fn heartbeat_with_control(
    registration: RelayControlRegistration,
    request: ReportRelayHealthRequest,
) -> Result<()> {
    let client = HttpControlClient::with_bearer_token(
        registration.control_url.clone(),
        registration.control_token.clone(),
    )?;
    client
        .report_relay_health(&registration.relay_id, request)
        .await?;
    Ok(())
}

async fn report_usage_with_control(
    registration: RelayControlRegistration,
    sessions: Vec<RelaySession>,
) -> Result<()> {
    let client = HttpControlClient::with_bearer_token(
        registration.control_url.clone(),
        registration.control_token.clone(),
    )?;
    client
        .report_relay_session_usage(usage_request_from_sessions(&registration, sessions))
        .await?;
    Ok(())
}

fn usage_request_from_sessions(
    registration: &RelayControlRegistration,
    sessions: Vec<RelaySession>,
) -> ReportRelaySessionUsageRequest {
    ReportRelaySessionUsageRequest {
        relay_id: registration.relay_id.clone(),
        sessions: sessions
            .into_iter()
            .map(|session| RelaySessionUsageReport {
                session_id: session.session_id,
                stats: session.stats,
            })
            .collect(),
    }
}

fn health_request_from_sessions(
    registration: &RelayControlRegistration,
    sessions: Vec<RelaySession>,
    uptime_sec: u64,
    data_plane_bound: bool,
    admin_bound: bool,
) -> ReportRelayHealthRequest {
    let health =
        health_report_from_sessions(sessions.clone(), uptime_sec, data_plane_bound, admin_bound);
    let snapshots = sessions
        .into_iter()
        .map(session_snapshot_from_relay_session)
        .collect();
    ReportRelayHealthRequest {
        relay_addr: registration.relay_addr.clone(),
        admin_addr: registration.admin_addr.clone(),
        capacity_streams: registration.capacity_streams,
        health,
        sessions: snapshots,
    }
}

fn session_snapshot_from_relay_session(session: RelaySession) -> RelaySessionSnapshot {
    RelaySessionSnapshot {
        session_id: session.session_id,
        state: relay_session_state_name(&session.state).to_string(),
        mobile_bound: session.mobile.is_some(),
        agent_bound: session.agent.is_some(),
        limits: session.limits,
        stats: session.stats,
        last_seen_epoch_sec: 0,
    }
}

fn relay_session_state_name(state: &RelaySessionState) -> &'static str {
    match state {
        RelaySessionState::Waiting => "waiting",
        RelaySessionState::Ready => "ready",
        RelaySessionState::Closed => "closed",
    }
}

fn health_report_from_sessions(
    sessions: Vec<RelaySession>,
    uptime_sec: u64,
    data_plane_bound: bool,
    admin_bound: bool,
) -> RelayHealthReport {
    let mut active_sessions = 0_u64;
    let mut active_streams = 0_u64;
    let mut total_uplink_bytes = 0_u64;
    let mut total_downlink_bytes = 0_u64;
    let mut total_bytes = 0_u64;

    for session in sessions {
        if session.state != RelaySessionState::Closed {
            active_sessions = active_sessions.saturating_add(1);
        }
        active_streams = active_streams.saturating_add(u64::from(session.stats.active_streams));
        total_uplink_bytes = total_uplink_bytes.saturating_add(session.stats.uplink_bytes);
        total_downlink_bytes = total_downlink_bytes.saturating_add(session.stats.downlink_bytes);
        total_bytes = total_bytes.saturating_add(session.stats.total_bytes);
    }

    let (status, reason) = if data_plane_bound {
        (RelayHealthStatus::Healthy, String::new())
    } else {
        (
            RelayHealthStatus::Unhealthy,
            "data_plane_not_bound".to_string(),
        )
    };

    RelayHealthReport {
        status,
        reason,
        relay_version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_sec,
        active_sessions,
        active_streams,
        total_uplink_bytes,
        total_downlink_bytes,
        total_bytes,
        data_plane_bound,
        admin_bound,
    }
}

async fn pending_relay_commands_with_control(
    registration: &RelayControlRegistration,
) -> Result<Vec<RelayCommand>> {
    let client = HttpControlClient::with_bearer_token(
        registration.control_url.clone(),
        registration.control_token.clone(),
    )?;
    Ok(client
        .pending_relay_commands(&registration.relay_id)
        .await?)
}

async fn report_relay_command_result_with_control(
    registration: &RelayControlRegistration,
    command_id: &str,
    request: ReportRelayCommandResultRequest,
) -> Result<()> {
    let client = HttpControlClient::with_bearer_token(
        registration.control_url.clone(),
        registration.control_token.clone(),
    )?;
    client
        .report_relay_command_result(&registration.relay_id, command_id, request)
        .await?;
    Ok(())
}

async fn process_pending_relay_commands(
    registration: RelayControlRegistration,
    session_store: RelaySessionStore,
) -> Result<()> {
    let commands = pending_relay_commands_with_control(&registration).await?;
    for command in commands {
        let result = execute_relay_command(&session_store, &command);
        if let Err(error) =
            report_relay_command_result_with_control(&registration, &command.command_id, result)
                .await
        {
            eprintln!(
                "relayd command result report failed for {} command {} at {}: {error:#}",
                registration.relay_id, command.command_id, registration.control_url
            );
        }
    }
    Ok(())
}

fn execute_relay_command(
    session_store: &RelaySessionStore,
    command: &RelayCommand,
) -> ReportRelayCommandResultRequest {
    match command.kind {
        RelayCommandKind::DisconnectSession => {
            let Some(session_id) = command.session_id.as_ref() else {
                return ReportRelayCommandResultRequest {
                    status: RelayCommandStatus::Failed,
                    message: "missing session_id".to_string(),
                };
            };
            match session_store.close(session_id) {
                Ok(_) => ReportRelayCommandResultRequest {
                    status: RelayCommandStatus::Succeeded,
                    message: "session closed".to_string(),
                },
                Err(error) => ReportRelayCommandResultRequest {
                    status: RelayCommandStatus::Failed,
                    message: error.to_string(),
                },
            }
        }
    }
}

async fn run_control_heartbeat_until(
    registration: RelayControlRegistration,
    interval: std::time::Duration,
    session_store: RelaySessionStore,
    data_plane_bound: bool,
    admin_bound: bool,
    started_at: std::time::Instant,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let sessions = session_store.list();
                let health = health_request_from_sessions(
                    &registration,
                    sessions.clone(),
                    started_at.elapsed().as_secs(),
                    data_plane_bound,
                    admin_bound,
                );
                if let Err(error) = heartbeat_with_control(registration.clone(), health).await {
                    eprintln!(
                        "relayd heartbeat failed for {} at {}: {error:#}",
                        registration.relay_id, registration.control_url
                    );
                }
                if let Err(error) = report_usage_with_control(
                    registration.clone(),
                    sessions,
                ).await {
                    eprintln!(
                        "relayd usage report failed for {} at {}: {error:#}",
                        registration.relay_id, registration.control_url
                    );
                }
                if let Err(error) = process_pending_relay_commands(
                    registration.clone(),
                    session_store.clone(),
                ).await {
                    eprintln!(
                        "relayd command poll failed for {} at {}: {error:#}",
                        registration.relay_id, registration.control_url
                    );
                }
            }
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
        }
    }
}

async fn wait_for_shutdown(mut shutdown_rx: watch::Receiver<bool>) {
    while !*shutdown_rx.borrow() {
        if shutdown_rx.changed().await.is_err() {
            break;
        }
    }
}

fn current_epoch_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before unix epoch")
        .as_secs()
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use quic_tunnel_auth::{RelayTokenClaims, TokenKey, TokenSigner};
    use quic_tunnel_control_client::{
        RelayCommand, RelayCommandKind, RelayCommandStatus, RelayHealthStatus,
    };
    use quic_tunnel_protocol::{
        ClientId, DeviceId, RelayLimits, ServiceId, SessionId, TrafficStats, UserId,
    };
    use quic_tunnel_relay::{
        bind::{
            RelayBindRequest, RelayBindStatus, RelayPeer, RelayPeerRole,
            SharedKeyRelayTokenVerifier,
        },
        session::RelaySessionState,
    };
    use std::sync::Arc;

    fn test_claims(session_id: &str) -> RelayTokenClaims {
        RelayTokenClaims {
            session_id: SessionId::new(session_id),
            user_id: UserId::new("user_001"),
            client_id: ClientId::new("mobile_001"),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
            max_bps: 2_097_152,
            max_streams: 32,
            max_duration_sec: 3_600,
            traffic_quota_bytes: 1_073_741_824,
            exp: 4_102_444_800,
        }
    }

    fn test_token(signer: &TokenSigner, session_id: &str) -> String {
        signer.sign_relay(&test_claims(session_id)).unwrap()
    }

    fn test_store() -> (RelaySessionStore, TokenSigner) {
        let signer = TokenSigner::new(TokenKey::new("dev-secret"));
        let verifier = SharedKeyRelayTokenVerifier::new(TokenKey::new("dev-secret"), 1_767_000_000);
        (RelaySessionStore::new(Arc::new(verifier)), signer)
    }

    fn bind_ready_session(store: &RelaySessionStore, signer: &TokenSigner, session_id: &str) {
        let session_id = SessionId::new(session_id);
        let session_id_value = session_id.as_str().to_string();
        assert_eq!(
            store
                .bind(RelayBindRequest {
                    role: RelayPeerRole::Mobile,
                    session_id: session_id.clone(),
                    token: test_token(signer, &session_id_value),
                })
                .unwrap(),
            RelayBindStatus::Waiting
        );
        assert_eq!(
            store
                .bind(RelayBindRequest {
                    role: RelayPeerRole::Agent,
                    session_id,
                    token: test_token(signer, &session_id_value),
                })
                .unwrap(),
            RelayBindStatus::Ready
        );
    }

    #[test]
    fn relayd_args_build_runtime_config() {
        let cli = Cli::try_parse_from([
            "relayd",
            "--bind",
            "127.0.0.1:4443",
            "--token-secret",
            "dev-secret",
            "--now-epoch-sec",
            "1767000000",
            "--debug-admin-listen",
            "127.0.0.1:9090",
            "--cert-out",
            "relay.der",
        ])
        .unwrap();

        assert_eq!(cli.bind.to_string(), "127.0.0.1:4443");
        assert_eq!(cli.config(None).unwrap().token_secret, "dev-secret");
        assert_eq!(cli.config(None).unwrap().now_epoch_sec, 1_767_000_000);
        assert_eq!(
            cli.debug_admin_listen().unwrap().to_string(),
            "127.0.0.1:9090"
        );
        assert_eq!(cli.cert_out.unwrap().to_string_lossy(), "relay.der");
    }

    #[test]
    fn relayd_args_build_control_registration() {
        let cli = Cli::try_parse_from([
            "relayd",
            "--bind",
            "127.0.0.1:0",
            "--token-secret",
            "dev-secret",
            "--control-url",
            "http://127.0.0.1:4242",
            "--control-token",
            "admin-token",
            "--relay-id",
            "relay_auto",
            "--capacity-streams",
            "64",
        ])
        .unwrap();

        let registration = cli
            .control_registration("127.0.0.1:4443".parse().unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(registration.control_url, "http://127.0.0.1:4242");
        assert_eq!(registration.control_token, "admin-token");
        assert_eq!(registration.relay_id, "relay_auto");
        assert_eq!(registration.relay_addr, "127.0.0.1:4443");
        assert_eq!(registration.admin_addr, "");
        assert_eq!(registration.capacity_streams, 64);
    }

    #[test]
    fn relayd_args_do_not_advertise_admin_addr_from_bound_debug_listener() {
        let cli = Cli::try_parse_from([
            "relayd",
            "--bind",
            "127.0.0.1:0",
            "--token-secret",
            "dev-secret",
            "--control-url",
            "http://127.0.0.1:4242",
            "--control-token",
            "relay-token",
            "--relay-id",
            "relay_auto",
        ])
        .unwrap();

        let registration = cli
            .control_registration("127.0.0.1:4443".parse().unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(registration.admin_addr, "");
    }

    #[test]
    fn relayd_debug_admin_listen_is_explicit_local_debug_only() {
        let cli = Cli::try_parse_from([
            "relayd",
            "--bind",
            "127.0.0.1:4443",
            "--token-secret",
            "dev-secret",
            "--debug-admin-listen",
            "127.0.0.1:9090",
        ])
        .unwrap();

        assert_eq!(
            cli.debug_admin_listen.unwrap().to_string(),
            "127.0.0.1:9090"
        );
    }

    #[test]
    fn relayd_bootstrap_args_build_exchange_request_and_runtime_config() {
        let cli = Cli::try_parse_from([
            "relayd",
            "--bind",
            "127.0.0.1:0",
            "--bootstrap-control-url",
            "https://control.example.com",
            "--bootstrap-id",
            "rb_001",
            "--bootstrap-token",
            "shown-once",
        ])
        .unwrap();

        let bootstrap = cli.bootstrap_exchange().unwrap().unwrap();
        assert_eq!(bootstrap.control_url, "https://control.example.com");
        assert_eq!(bootstrap.bootstrap_id, "rb_001");
        assert_eq!(bootstrap.bootstrap_token, "shown-once");

        let response = quic_tunnel_control_client::RelayBootstrapExchangeResponse {
            control_url: "https://control.example.com".to_string(),
            control_token: "relay-control-token".to_string(),
            relay_id: "relay_bootstrap".to_string(),
            token_secret: "relay-data-plane-secret".to_string(),
            relay_addr: "relay.example.com:4443".to_string(),
            admin_addr: "127.0.0.1:9090".to_string(),
            capacity_streams: 64,
            heartbeat_interval_sec: 15,
        };

        let config = cli.config(Some(&response)).unwrap();
        assert_eq!(config.token_secret, "relay-data-plane-secret");
        let registration = RelayControlRegistration::from_bootstrap(response);
        assert_eq!(registration.control_url, "https://control.example.com");
        assert_eq!(registration.control_token, "relay-control-token");
        assert_eq!(registration.relay_id, "relay_bootstrap");
        assert_eq!(registration.relay_addr, "relay.example.com:4443");
        assert_eq!(registration.admin_addr, "");
        assert_eq!(registration.capacity_streams, 64);
    }

    #[test]
    fn bootstrap_registration_ignores_legacy_admin_addr_from_control_response() {
        let response = quic_tunnel_control_client::RelayBootstrapExchangeResponse {
            control_url: "https://control.example.com".to_string(),
            control_token: "relay-control-token".to_string(),
            relay_id: "relay_bootstrap".to_string(),
            token_secret: "relay-data-plane-secret".to_string(),
            relay_addr: "relay.example.com:4443".to_string(),
            admin_addr: "legacy-admin.example.com:9090".to_string(),
            capacity_streams: 64,
            heartbeat_interval_sec: 15,
        };

        let registration = RelayControlRegistration::from_bootstrap(response);
        assert_eq!(registration.admin_addr, "");
    }

    #[test]
    fn relayd_args_build_heartbeat_interval() {
        let cli = Cli::try_parse_from([
            "relayd",
            "--bind",
            "127.0.0.1:0",
            "--token-secret",
            "dev-secret",
            "--control-url",
            "http://127.0.0.1:4242",
            "--control-token",
            "admin-token",
            "--relay-id",
            "relay_auto",
            "--heartbeat-interval-sec",
            "5",
        ])
        .unwrap();

        assert_eq!(cli.heartbeat_interval(), std::time::Duration::from_secs(5));
    }

    #[test]
    fn usage_request_from_sessions_maps_relay_stats() {
        let registration = RelayControlRegistration {
            control_url: "http://127.0.0.1:4242".to_string(),
            control_token: "relay-token".to_string(),
            relay_id: "relay_usage".to_string(),
            relay_addr: "127.0.0.1:4443".to_string(),
            admin_addr: "127.0.0.1:9090".to_string(),
            capacity_streams: 64,
        };
        let session_id = SessionId::new("sess_usage");
        let request = usage_request_from_sessions(
            &registration,
            vec![RelaySession {
                session_id: session_id.clone(),
                mobile: Some(RelayPeer::new(RelayPeerRole::Mobile)),
                agent: Some(RelayPeer::new(RelayPeerRole::Agent)),
                limits: RelayLimits {
                    max_bps: 1024,
                    max_streams: 4,
                    max_duration_sec: 60,
                    traffic_quota_bytes: 4096,
                },
                stats: TrafficStats {
                    session_id: Some(session_id.clone()),
                    uplink_bytes: 13,
                    downlink_bytes: 21,
                    total_bytes: 34,
                    duration_sec: 5,
                    active_streams: 1,
                },
                started_at: std::time::Instant::now(),
                state: RelaySessionState::Ready,
            }],
        );

        assert_eq!(request.relay_id, "relay_usage");
        assert_eq!(request.sessions.len(), 1);
        assert_eq!(request.sessions[0].session_id, session_id);
        assert_eq!(request.sessions[0].stats.uplink_bytes, 13);
        assert_eq!(request.sessions[0].stats.downlink_bytes, 21);
        assert_eq!(request.sessions[0].stats.total_bytes, 34);
    }

    #[test]
    fn health_report_from_sessions_summarizes_runtime_metrics() {
        let ready_session_id = SessionId::new("sess_ready");
        let closed_session_id = SessionId::new("sess_closed");
        let report = health_report_from_sessions(
            vec![
                RelaySession {
                    session_id: ready_session_id.clone(),
                    mobile: Some(RelayPeer::new(RelayPeerRole::Mobile)),
                    agent: Some(RelayPeer::new(RelayPeerRole::Agent)),
                    limits: RelayLimits {
                        max_bps: 1024,
                        max_streams: 4,
                        max_duration_sec: 60,
                        traffic_quota_bytes: 4096,
                    },
                    stats: TrafficStats {
                        session_id: Some(ready_session_id),
                        uplink_bytes: 13,
                        downlink_bytes: 21,
                        total_bytes: 34,
                        duration_sec: 5,
                        active_streams: 2,
                    },
                    started_at: std::time::Instant::now(),
                    state: RelaySessionState::Ready,
                },
                RelaySession {
                    session_id: closed_session_id.clone(),
                    mobile: None,
                    agent: None,
                    limits: RelayLimits {
                        max_bps: 1024,
                        max_streams: 4,
                        max_duration_sec: 60,
                        traffic_quota_bytes: 4096,
                    },
                    stats: TrafficStats {
                        session_id: Some(closed_session_id),
                        uplink_bytes: 100,
                        downlink_bytes: 200,
                        total_bytes: 300,
                        duration_sec: 5,
                        active_streams: 0,
                    },
                    started_at: std::time::Instant::now(),
                    state: RelaySessionState::Closed,
                },
            ],
            45,
            true,
            true,
        );

        assert_eq!(report.status, RelayHealthStatus::Healthy);
        assert_eq!(report.reason, "");
        assert_eq!(report.uptime_sec, 45);
        assert_eq!(report.active_sessions, 1);
        assert_eq!(report.active_streams, 2);
        assert_eq!(report.total_uplink_bytes, 113);
        assert_eq!(report.total_downlink_bytes, 221);
        assert_eq!(report.total_bytes, 334);
        assert!(report.data_plane_bound);
        assert!(report.admin_bound);
        assert_eq!(report.relay_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn health_report_marks_missing_data_plane_unhealthy() {
        let report = health_report_from_sessions(Vec::new(), 1, false, true);

        assert_eq!(report.status, RelayHealthStatus::Unhealthy);
        assert_eq!(report.reason, "data_plane_not_bound");
        assert!(!report.data_plane_bound);
        assert!(report.admin_bound);
    }

    #[test]
    fn relay_live_ops_health_request_includes_session_snapshots() {
        let registration = RelayControlRegistration {
            control_url: "http://127.0.0.1:4242".to_string(),
            control_token: "relay-token".to_string(),
            relay_id: "relay_live_ops".to_string(),
            relay_addr: "127.0.0.1:4443".to_string(),
            admin_addr: String::new(),
            capacity_streams: 64,
        };
        let session_id = SessionId::new("sess_live_ops_snapshot");
        let request = health_request_from_sessions(
            &registration,
            vec![RelaySession {
                session_id: session_id.clone(),
                mobile: Some(RelayPeer::new(RelayPeerRole::Mobile)),
                agent: Some(RelayPeer::new(RelayPeerRole::Agent)),
                limits: RelayLimits {
                    max_bps: 1024,
                    max_streams: 4,
                    max_duration_sec: 60,
                    traffic_quota_bytes: 4096,
                },
                stats: TrafficStats {
                    session_id: Some(session_id.clone()),
                    uplink_bytes: 13,
                    downlink_bytes: 21,
                    total_bytes: 34,
                    duration_sec: 5,
                    active_streams: 2,
                },
                started_at: std::time::Instant::now(),
                state: RelaySessionState::Ready,
            }],
            45,
            true,
            false,
        );

        assert_eq!(request.relay_addr, "127.0.0.1:4443");
        assert_eq!(request.capacity_streams, 64);
        assert_eq!(request.health.active_sessions, 1);
        assert_eq!(request.health.active_streams, 2);
        assert_eq!(request.sessions.len(), 1);
        assert_eq!(request.sessions[0].session_id, session_id);
        assert_eq!(request.sessions[0].state, "ready");
        assert!(request.sessions[0].mobile_bound);
        assert!(request.sessions[0].agent_bound);
        assert_eq!(request.sessions[0].stats.total_bytes, 34);
    }

    #[test]
    fn relay_live_ops_disconnect_command_closes_local_session() {
        let (store, signer) = test_store();
        bind_ready_session(&store, &signer, "sess_live_ops_close");
        let command = RelayCommand {
            command_id: "rc_close".to_string(),
            relay_id: "relay_live_ops".to_string(),
            kind: RelayCommandKind::DisconnectSession,
            session_id: Some(SessionId::new("sess_live_ops_close")),
            status: RelayCommandStatus::Pending,
            requested_epoch_sec: 1,
            updated_epoch_sec: 1,
            message: String::new(),
        };

        let result = execute_relay_command(&store, &command);

        assert_eq!(result.status, RelayCommandStatus::Succeeded);
        assert_eq!(result.message, "session closed");
        assert_eq!(
            store
                .get(&SessionId::new("sess_live_ops_close"))
                .unwrap()
                .state,
            RelaySessionState::Closed
        );
    }

    #[test]
    fn relay_live_ops_disconnect_command_reports_missing_session() {
        let (store, _signer) = test_store();
        let command = RelayCommand {
            command_id: "rc_missing".to_string(),
            relay_id: "relay_live_ops".to_string(),
            kind: RelayCommandKind::DisconnectSession,
            session_id: Some(SessionId::new("sess_missing")),
            status: RelayCommandStatus::Pending,
            requested_epoch_sec: 1,
            updated_epoch_sec: 1,
            message: String::new(),
        };

        let result = execute_relay_command(&store, &command);

        assert_eq!(result.status, RelayCommandStatus::Failed);
        assert!(result.message.contains("session not found"));
    }

    #[tokio::test]
    #[ignore = "binds a local TCP listener; run outside the sandbox"]
    async fn relayd_registers_itself_with_control() {
        let state = quic_tunnel_control::state::ControlState::new(
            "dev-secret",
            "seed-relay.example.com:4443",
            "punch.example.com:3478",
        );
        let admin_token = state.issue_admin_token("admin@example.com").unwrap();
        let relay_token = state.issue_relay_token("relay_auto").unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let control_addr = listener.local_addr().unwrap();
        let control_url = format!("http://{control_addr}");
        let server = tokio::spawn(async move {
            axum::serve(listener, quic_tunnel_control::routes::routes(state))
                .await
                .unwrap();
        });
        let registration = RelayControlRegistration {
            control_url: control_url.clone(),
            control_token: relay_token,
            relay_id: "relay_auto".to_string(),
            relay_addr: "127.0.0.1:4443".to_string(),
            admin_addr: String::new(),
            capacity_streams: 64,
        };

        register_with_control(registration).await.unwrap();

        let client = quic_tunnel_control_client::HttpControlClient::with_bearer_token(
            control_url,
            admin_token,
        )
        .unwrap();
        let relay = client.relay("relay_auto").await.unwrap();
        assert_eq!(relay.relay_addr, "127.0.0.1:4443");
        assert_eq!(relay.admin_addr, "");
        assert_eq!(relay.capacity_streams, 64);

        server.abort();
    }
}
