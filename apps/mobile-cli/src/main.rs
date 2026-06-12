use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use quic_tunnel_auth::ControlRole;
use quic_tunnel_control_client::{
    AdminListQuery, AssignUserPlanRequest, CreateRelayBootstrapRequest, CreateUserRequest,
    GrantDeviceAccessRequest, HttpControlClient, HttpControlClientOptions, RegisterRelayRequest,
    UpdateRelayRequest, UpdateUserStatusRequest,
};
use quic_tunnel_mobile_core::{
    client::{OpenServiceRequest, TunnelClient},
    config::TunnelConfig,
    forward::{RelayConnectorConfig, RelayStreamConnector, StreamConnector},
};
use quic_tunnel_protocol::{
    ClientId, DeviceId, MobileGrantCredential, MobileInvitePayload, ServiceId, SessionId, UserId,
};
use quic_tunnel_sdk::{
    mobile::{
        MobileGrantPairingInput, MobileTunnelConfig, MobileTunnelSdk, OpenServiceInput,
        P2pOrRelayTunnelConfig,
    },
    store::{FileMobileGrantStore, MemoryTokenStore, StoredToken, TokenStore},
};
use rustls::pki_types::CertificateDer;
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "mobile-cli")]
#[command(about = "Test mobile client for MobileCode Connect")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    OpenService(OpenServiceArgs),
    Pair(PairArgs),
    Admin(AdminArgs),
}

#[derive(Debug, Parser)]
struct AdminArgs {
    #[command(subcommand)]
    command: AdminCommand,
}

#[derive(Debug, Subcommand)]
enum AdminCommand {
    Users(AdminListCommandArgs),
    Audit(AdminListCommandArgs),
    Usage(AdminListCommandArgs),
    Sessions(AdminListCommandArgs),
    Controllers(AdminListCommandArgs),
    Devices(AdminListCommandArgs),
    PlanCatalog(AdminListCommandArgs),
    RelayCredentials(AdminListCommandArgs),
    Relays(AdminListCommandArgs),
    DeviceAccess(AdminDeviceAccessArgs),
    CreateUser(AdminCreateUserArgs),
    SetUserStatus(AdminSetUserStatusArgs),
    AssignPlan(AdminAssignPlanArgs),
    CreateRelayBootstrap(AdminCreateRelayBootstrapArgs),
    RegisterRelay(AdminRegisterRelayArgs),
    UpdateRelay(AdminUpdateRelayArgs),
    GrantDeviceAccess(AdminGrantDeviceAccessArgs),
    RevokeDeviceAccess(AdminRevokeDeviceAccessArgs),
}

#[derive(Debug, Args, Clone)]
struct AdminCommonArgs {
    #[arg(long)]
    control: String,
    #[arg(long)]
    token: String,
}

impl AdminCommonArgs {
    fn client(&self) -> Result<HttpControlClient> {
        HttpControlClient::with_bearer_token(&self.control, &self.token)
            .context("create control admin client")
    }
}

#[derive(Debug, Args, Clone)]
struct AdminListCommandArgs {
    #[command(flatten)]
    common: AdminCommonArgs,
    #[command(flatten)]
    query: AdminListArgs,
}

#[derive(Debug, Args, Clone, Default)]
struct AdminListArgs {
    #[arg(long)]
    limit: Option<u32>,
    #[arg(long)]
    offset: Option<u32>,
    #[arg(long)]
    sort: Option<String>,
    #[arg(long)]
    q: Option<String>,
    #[arg(long)]
    role: Option<String>,
    #[arg(long)]
    enabled: Option<bool>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    user_id: Option<String>,
    #[arg(long)]
    device_id: Option<String>,
    #[arg(long)]
    healthy: Option<bool>,
    #[arg(long)]
    action: Option<String>,
    #[arg(long)]
    target_type: Option<String>,
}

impl AdminListArgs {
    fn to_query(&self) -> AdminListQuery {
        AdminListQuery {
            limit: self.limit,
            offset: self.offset,
            sort: self.sort.clone(),
            q: self.q.clone(),
            role: self.role.clone(),
            enabled: self.enabled,
            status: self.status.clone(),
            user_id: self.user_id.as_deref().map(UserId::new),
            device_id: self.device_id.as_deref().map(DeviceId::new),
            healthy: self.healthy,
            action: self.action.clone(),
            target_type: self.target_type.clone(),
        }
    }
}

#[derive(Debug, Args)]
struct AdminDeviceAccessArgs {
    #[command(flatten)]
    common: AdminCommonArgs,
    #[arg(long)]
    device: String,
    #[command(flatten)]
    query: AdminListArgs,
}

#[derive(Debug, Args)]
struct AdminCreateUserArgs {
    #[command(flatten)]
    common: AdminCommonArgs,
    #[arg(long)]
    email: String,
    #[arg(long)]
    password: String,
    #[arg(long)]
    name: String,
    #[arg(long, default_value = "user", value_parser = parse_user_role)]
    role: ControlRole,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    enabled: bool,
}

impl AdminCreateUserArgs {
    fn request(&self) -> CreateUserRequest {
        CreateUserRequest {
            email: self.email.clone(),
            password: self.password.clone(),
            display_name: self.name.clone(),
            role: self.role,
            enabled: self.enabled,
        }
    }
}

#[derive(Debug, Args)]
struct AdminSetUserStatusArgs {
    #[command(flatten)]
    common: AdminCommonArgs,
    #[arg(long, value_parser = parse_user_id)]
    user_id: UserId,
    #[arg(long, action = clap::ArgAction::Set)]
    enabled: bool,
}

impl AdminSetUserStatusArgs {
    fn request(&self) -> UpdateUserStatusRequest {
        UpdateUserStatusRequest {
            enabled: self.enabled,
        }
    }
}

#[derive(Debug, Args)]
struct AdminAssignPlanArgs {
    #[command(flatten)]
    common: AdminCommonArgs,
    #[arg(long, value_parser = parse_user_id)]
    user_id: UserId,
    #[arg(long)]
    plan_id: String,
}

impl AdminAssignPlanArgs {
    fn request(&self) -> AssignUserPlanRequest {
        AssignUserPlanRequest {
            plan_id: self.plan_id.clone(),
        }
    }
}

#[derive(Debug, Args)]
struct AdminRegisterRelayArgs {
    #[command(flatten)]
    common: AdminCommonArgs,
    #[arg(long)]
    relay_id: String,
    #[arg(long)]
    relay_addr: String,
    #[arg(long)]
    admin_addr: String,
    #[arg(long)]
    capacity_streams: u32,
}

impl AdminRegisterRelayArgs {
    fn request(&self) -> RegisterRelayRequest {
        RegisterRelayRequest {
            relay_id: self.relay_id.clone(),
            relay_addr: self.relay_addr.clone(),
            admin_addr: self.admin_addr.clone(),
            capacity_streams: self.capacity_streams,
        }
    }
}

#[derive(Debug, Args)]
struct AdminCreateRelayBootstrapArgs {
    #[command(flatten)]
    common: AdminCommonArgs,
    #[arg(long)]
    relay_id: String,
    #[arg(long)]
    relay_addr: String,
    #[arg(long, default_value = "")]
    admin_addr: String,
    #[arg(long)]
    capacity_streams: u32,
    #[arg(long, default_value_t = 30)]
    heartbeat_interval_sec: u64,
    #[arg(long, default_value_t = 900)]
    ttl_sec: u64,
}

impl AdminCreateRelayBootstrapArgs {
    fn request(&self) -> CreateRelayBootstrapRequest {
        CreateRelayBootstrapRequest {
            relay_id: self.relay_id.clone(),
            control_url: self.common.control.clone(),
            relay_addr: self.relay_addr.clone(),
            admin_addr: self.admin_addr.clone(),
            capacity_streams: self.capacity_streams,
            heartbeat_interval_sec: self.heartbeat_interval_sec,
            ttl_sec: self.ttl_sec,
        }
    }
}

#[derive(Debug, Args)]
struct AdminUpdateRelayArgs {
    #[command(flatten)]
    common: AdminCommonArgs,
    #[arg(long)]
    relay_id: String,
    #[arg(long)]
    relay_addr: String,
    #[arg(long)]
    admin_addr: String,
    #[arg(long)]
    capacity_streams: u32,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    healthy: bool,
}

impl AdminUpdateRelayArgs {
    fn request(&self) -> UpdateRelayRequest {
        UpdateRelayRequest {
            relay_addr: self.relay_addr.clone(),
            admin_addr: self.admin_addr.clone(),
            capacity_streams: self.capacity_streams,
            healthy: self.healthy,
        }
    }
}

#[derive(Debug, Args)]
struct AdminGrantDeviceAccessArgs {
    #[command(flatten)]
    common: AdminCommonArgs,
    #[arg(long, value_parser = parse_device_id)]
    device: DeviceId,
    #[arg(long, value_parser = parse_user_id)]
    user_id: UserId,
}

impl AdminGrantDeviceAccessArgs {
    fn request(&self) -> GrantDeviceAccessRequest {
        GrantDeviceAccessRequest {
            user_id: self.user_id.clone(),
        }
    }
}

#[derive(Debug, Args)]
struct AdminRevokeDeviceAccessArgs {
    #[command(flatten)]
    common: AdminCommonArgs,
    #[arg(long, value_parser = parse_device_id)]
    device: DeviceId,
    #[arg(long, value_parser = parse_user_id)]
    user_id: UserId,
}

#[derive(Debug, Args)]
struct PairArgs {
    #[arg(long = "invite-file")]
    invite_file: PathBuf,
    #[arg(long = "grant-file")]
    grant_file: PathBuf,
    #[arg(long)]
    client: String,
    #[arg(long = "service", value_parser = parse_service_id)]
    services: Vec<ServiceId>,
    #[arg(long = "nonce", default_value = "mobile-cli-pairing")]
    nonce: String,
    #[arg(long = "poll-ms", default_value_t = 1000)]
    poll_ms: u64,
    #[arg(long = "control-request-timeout-ms")]
    control_request_timeout_ms: Option<u64>,
    #[arg(long = "control-max-retries", default_value_t = 0)]
    control_max_retries: u32,
    #[arg(long = "control-retry-backoff-ms", default_value_t = 0)]
    control_retry_backoff_ms: u64,
}

impl PairArgs {
    fn control_client_options(&self) -> HttpControlClientOptions {
        let options = HttpControlClientOptions::default()
            .with_max_retries(self.control_max_retries)
            .with_retry_backoff(Duration::from_millis(self.control_retry_backoff_ms));
        if let Some(timeout_ms) = self.control_request_timeout_ms {
            options.with_request_timeout(Duration::from_millis(timeout_ms))
        } else {
            options
        }
    }
}

#[derive(Debug, Parser)]
struct OpenServiceArgs {
    #[arg(long)]
    control: String,
    #[arg(long)]
    token: Option<String>,
    #[arg(long = "grant-file", conflicts_with = "token")]
    grant_file: Option<PathBuf>,
    #[arg(long)]
    client: String,
    #[arg(long)]
    device: String,
    #[arg(long)]
    service: String,
    #[arg(long)]
    local: u16,
    #[arg(long = "relay")]
    relay_addr: Option<SocketAddr>,
    #[arg(long = "relay-cert")]
    relay_cert: PathBuf,
    #[arg(long = "session", value_parser = parse_session_id)]
    session_id: Option<SessionId>,
    #[arg(long = "relay-token")]
    relay_token: Option<String>,
    #[arg(long = "p2p-bind", default_value = "0.0.0.0:0")]
    p2p_bind_addr: SocketAddr,
    #[arg(long = "p2p-candidate-timeout-ms", default_value_t = 1500)]
    p2p_candidate_timeout_ms: u64,
    #[arg(long = "p2p-probe-timeout-ms", default_value_t = 1500)]
    p2p_probe_timeout_ms: u64,
    #[arg(long = "p2p-interval-ms", default_value_t = 25)]
    p2p_interval_ms: u64,
    #[arg(long = "relay-fallback-delay-ms", default_value_t = 300)]
    relay_fallback_delay_ms: u64,
    #[arg(long = "control-request-timeout-ms")]
    control_request_timeout_ms: Option<u64>,
    #[arg(long = "control-max-retries", default_value_t = 0)]
    control_max_retries: u32,
    #[arg(long = "control-retry-backoff-ms", default_value_t = 0)]
    control_retry_backoff_ms: u64,
}

impl OpenServiceArgs {
    fn sdk_tunnel_config(&self) -> MobileTunnelConfig {
        MobileTunnelConfig {
            control_server_url: self.control.clone(),
            client_id: ClientId::new(self.client.clone()),
            control_client_options: self.control_client_options(),
        }
    }

    fn tunnel_config(&self) -> TunnelConfig {
        TunnelConfig {
            user_token: self.token.clone().unwrap_or_default(),
            control_server_url: self.control.clone(),
            client_id: ClientId::new(self.client.clone()),
            control_client_options: self.control_client_options(),
        }
    }

    fn control_client_options(&self) -> HttpControlClientOptions {
        let options = HttpControlClientOptions::default()
            .with_max_retries(self.control_max_retries)
            .with_retry_backoff(Duration::from_millis(self.control_retry_backoff_ms));
        if let Some(timeout_ms) = self.control_request_timeout_ms {
            options.with_request_timeout(Duration::from_millis(timeout_ms))
        } else {
            options
        }
    }

    fn open_input(&self) -> OpenServiceInput {
        OpenServiceInput {
            device_id: DeviceId::new(self.device.clone()),
            service_id: ServiceId::new(self.service.clone()),
            local_port: self.local,
        }
    }

    fn open_request(&self) -> OpenServiceRequest {
        OpenServiceRequest {
            device_id: DeviceId::new(self.device.clone()),
            service_id: ServiceId::new(self.service.clone()),
            local_port: self.local,
        }
    }

    async fn token_store(&self) -> Result<MemoryTokenStore> {
        let token = self
            .token
            .clone()
            .context("--token is required unless --grant-file is provided")?;
        let store = MemoryTokenStore::default();
        store
            .save_token(StoredToken {
                user_id: UserId::new("mobile-cli"),
                access_token: token,
                expire_at: 0,
            })
            .await
            .context("save CLI token for SDK")?;
        Ok(store)
    }

    async fn mobile_grant(&self) -> Result<Option<MobileGrantCredential>> {
        let Some(path) = &self.grant_file else {
            return Ok(None);
        };
        let body = tokio::fs::read(path)
            .await
            .with_context(|| format!("read mobile grant {}", path.display()))?;
        let grant = serde_json::from_slice(&body)
            .with_context(|| format!("parse mobile grant {}", path.display()))?;
        Ok(Some(grant))
    }

    fn direct_relay_config(
        &self,
        server_cert: CertificateDer<'static>,
    ) -> Result<Option<RelayConnectorConfig>> {
        match (&self.relay_addr, &self.session_id, &self.relay_token) {
            (Some(relay_addr), Some(session_id), Some(relay_token)) => {
                Ok(Some(RelayConnectorConfig {
                    relay_addr: *relay_addr,
                    server_cert,
                    session_id: session_id.clone(),
                    token: relay_token.clone(),
                }))
            }
            (None, None, None) => Ok(None),
            _ => anyhow::bail!(
                "--relay, --session, and --relay-token must be provided together for direct Relay mode"
            ),
        }
    }

    fn sdk_p2p_or_relay_config(
        &self,
        server_cert: CertificateDer<'static>,
    ) -> P2pOrRelayTunnelConfig {
        P2pOrRelayTunnelConfig {
            relay_server_cert: server_cert,
            bind_addr: self.p2p_bind_addr,
            candidate_timeout: Duration::from_millis(self.p2p_candidate_timeout_ms),
            probe_timeout: Duration::from_millis(self.p2p_probe_timeout_ms),
            interval: Duration::from_millis(self.p2p_interval_ms),
            relay_fallback_delay: Duration::from_millis(self.relay_fallback_delay_ms),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    match cli.command {
        Commands::OpenService(args) => {
            let server_cert =
                CertificateDer::from(tokio::fs::read(&args.relay_cert).await.with_context(
                    || format!("read relay certificate {}", args.relay_cert.display()),
                )?);
            if let Some(relay_config) = args.direct_relay_config(server_cert.clone())? {
                let connector: Arc<dyn StreamConnector> = Arc::new(
                    RelayStreamConnector::connect(relay_config)
                        .await
                        .context("connect mobile relay stream connector")?,
                );
                let client = TunnelClient::with_connector(args.tunnel_config(), connector)
                    .await
                    .context("start tunnel client")?;
                let handle = client
                    .open_service(args.open_request())
                    .await
                    .context("open local service forward")?;
                println!("mobile-cli forwarding on 127.0.0.1:{}", handle.local_port());
                tokio::signal::ctrl_c().await.context("wait for ctrl-c")?;
                client
                    .close_service(handle.handle_id().to_string())
                    .await
                    .context("close local service forward")?;
            } else {
                let p2p_or_relay = args.sdk_p2p_or_relay_config(server_cert);
                let tunnel = if let Some(grant) = args.mobile_grant().await? {
                    MobileTunnelSdk::start_with_mobile_grant(
                        args.sdk_tunnel_config(),
                        grant,
                        p2p_or_relay,
                    )
                    .await
                    .context("start SDK mobile tunnel with grant")?
                } else {
                    MobileTunnelSdk::start_with_control_p2p_or_relay(
                        args.sdk_tunnel_config(),
                        args.token_store().await?,
                        p2p_or_relay,
                    )
                    .await
                    .context("start SDK mobile tunnel")?
                };
                let handle = tunnel
                    .open_service(args.open_input())
                    .await
                    .context("open local service forward")?;
                println!("mobile-cli forwarding on 127.0.0.1:{}", handle.local_port());
                tokio::signal::ctrl_c().await.context("wait for ctrl-c")?;
                tunnel
                    .close_service(handle.handle_id().to_string())
                    .await
                    .context("close local service forward")?;
            }
        }
        Commands::Pair(args) => pair_mobile(args).await?,
        Commands::Admin(args) => run_admin(args).await?,
    }

    Ok(())
}

async fn pair_mobile(args: PairArgs) -> Result<()> {
    if args.services.is_empty() {
        anyhow::bail!("at least one --service is required");
    }
    let body = tokio::fs::read(&args.invite_file)
        .await
        .with_context(|| format!("read invite {}", args.invite_file.display()))?;
    let invite: MobileInvitePayload = serde_json::from_slice(&body)
        .with_context(|| format!("parse invite {}", args.invite_file.display()))?;
    let options = args.control_client_options();
    let pairing = MobileTunnelSdk::start_mobile_grant_pairing(
        MobileGrantPairingInput {
            invite,
            client_id: ClientId::new(args.client.clone()),
            requested_services: args.services.clone(),
            nonce: args.nonce.clone(),
        },
        options,
    )
    .await
    .context("start mobile grant pairing")?;
    let store = FileMobileGrantStore::new(args.grant_file.clone());
    let poll_delay = Duration::from_millis(pairing.poll_interval_ms.max(args.poll_ms).max(1));
    loop {
        if let Some(grant) = MobileTunnelSdk::complete_mobile_grant_pairing_once(
            pairing.clone(),
            store.clone(),
            args.control_client_options(),
        )
        .await
        .context("poll mobile grant pairing")?
        {
            print_json(&grant)?;
            return Ok(());
        }
        tokio::time::sleep(poll_delay).await;
    }
}

async fn run_admin(args: AdminArgs) -> Result<()> {
    match args.command {
        AdminCommand::Users(args) => {
            let client = args.common.client()?;
            print_json(&client.list_users_with_query(args.query.to_query()).await?)
        }
        AdminCommand::Audit(args) => {
            let client = args.common.client()?;
            print_json(&client.audit_logs_with_query(args.query.to_query()).await?)
        }
        AdminCommand::Usage(args) => {
            let client = args.common.client()?;
            print_json(
                &client
                    .user_usage_summaries_with_query(args.query.to_query())
                    .await?,
            )
        }
        AdminCommand::Sessions(args) => {
            let client = args.common.client()?;
            print_json(
                &client
                    .admin_sessions_with_query(args.query.to_query())
                    .await?,
            )
        }
        AdminCommand::Controllers(args) => {
            let client = args.common.client()?;
            print_json(
                &client
                    .list_controllers_with_query(args.query.to_query())
                    .await?,
            )
        }
        AdminCommand::Devices(args) => {
            let client = args.common.client()?;
            print_json(
                &client
                    .list_controlled_devices_with_query(args.query.to_query())
                    .await?,
            )
        }
        AdminCommand::PlanCatalog(args) => {
            let client = args.common.client()?;
            print_json(
                &client
                    .plan_catalog_with_query(args.query.to_query())
                    .await?,
            )
        }
        AdminCommand::RelayCredentials(args) => {
            let client = args.common.client()?;
            print_json(
                &client
                    .list_relay_credentials_with_query(args.query.to_query())
                    .await?,
            )
        }
        AdminCommand::Relays(args) => {
            let client = args.common.client()?;
            print_json(&client.list_relays_with_query(args.query.to_query()).await?)
        }
        AdminCommand::DeviceAccess(args) => print_json(
            &args
                .common
                .client()?
                .device_access_grants_with_query(&DeviceId::new(args.device), args.query.to_query())
                .await?,
        ),
        AdminCommand::CreateUser(args) => {
            let client = args.common.client()?;
            print_json(&client.create_user(args.request()).await?)
        }
        AdminCommand::SetUserStatus(args) => {
            let client = args.common.client()?;
            print_json(
                &client
                    .update_user_status(&args.user_id, args.request())
                    .await?,
            )
        }
        AdminCommand::AssignPlan(args) => {
            let client = args.common.client()?;
            print_json(
                &client
                    .assign_user_plan(&args.user_id, args.request())
                    .await?,
            )
        }
        AdminCommand::CreateRelayBootstrap(args) => {
            let client = args.common.client()?;
            print_json(&client.create_relay_bootstrap(args.request()).await?)
        }
        AdminCommand::RegisterRelay(args) => {
            let client = args.common.client()?;
            print_json(&client.register_relay(args.request()).await?)
        }
        AdminCommand::UpdateRelay(args) => {
            let client = args.common.client()?;
            print_json(&client.update_relay(&args.relay_id, args.request()).await?)
        }
        AdminCommand::GrantDeviceAccess(args) => {
            let client = args.common.client()?;
            print_json(
                &client
                    .grant_device_access(&args.device, args.request())
                    .await?,
            )
        }
        AdminCommand::RevokeDeviceAccess(args) => {
            args.common
                .client()?
                .revoke_device_access(&args.device, &args.user_id)
                .await?;
            println!("{{\"ok\":true}}");
            Ok(())
        }
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn parse_session_id(value: &str) -> std::result::Result<SessionId, String> {
    if value.trim().is_empty() {
        return Err("session id must not be empty".to_string());
    }
    Ok(SessionId::new(value))
}

fn parse_user_id(value: &str) -> std::result::Result<UserId, String> {
    if value.trim().is_empty() {
        return Err("user id must not be empty".to_string());
    }
    Ok(UserId::new(value))
}

fn parse_device_id(value: &str) -> std::result::Result<DeviceId, String> {
    if value.trim().is_empty() {
        return Err("device id must not be empty".to_string());
    }
    Ok(DeviceId::new(value))
}

fn parse_service_id(value: &str) -> std::result::Result<ServiceId, String> {
    if value.trim().is_empty() {
        return Err("service id must not be empty".to_string());
    }
    Ok(ServiceId::new(value))
}

fn parse_user_role(value: &str) -> std::result::Result<ControlRole, String> {
    match value {
        "user" => Ok(ControlRole::User),
        "admin" => Ok(ControlRole::Admin),
        _ => Err("role must be user or admin".to_string()),
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_service_args_build_sdk_config_and_request_for_control_mode() {
        let cli = Cli::try_parse_from([
            "mobile-cli",
            "open-service",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "user-token",
            "--client",
            "mobile_001",
            "--device",
            "pc_001",
            "--service",
            "svc_web_3000",
            "--local",
            "18080",
            "--relay-cert",
            "relay.der",
            "--control-request-timeout-ms",
            "5000",
            "--control-max-retries",
            "2",
            "--control-retry-backoff-ms",
            "25",
        ])
        .unwrap();

        let Commands::OpenService(args) = cli.command else {
            panic!("expected open-service command");
        };
        let config = args.sdk_tunnel_config();
        let request = args.open_input();

        assert_eq!(config.control_server_url, "http://127.0.0.1:4242");
        assert_eq!(config.client_id.as_str(), "mobile_001");
        assert_eq!(
            config
                .control_client_options
                .request_timeout()
                .unwrap()
                .as_millis(),
            5000
        );
        assert_eq!(config.control_client_options.max_retries(), 2);
        assert_eq!(
            config.control_client_options.retry_backoff().as_millis(),
            25
        );
        assert_eq!(
            args.tunnel_config()
                .control_client_options
                .request_timeout()
                .unwrap()
                .as_millis(),
            5000
        );
        assert_eq!(request.device_id.as_str(), "pc_001");
        assert_eq!(request.service_id.as_str(), "svc_web_3000");
        assert_eq!(request.local_port, 18_080);
        assert_eq!(args.relay_addr, None);
        assert_eq!(args.relay_cert.to_string_lossy(), "relay.der");
        assert_eq!(args.session_id, None);
        assert_eq!(args.relay_token, None);
    }

    #[test]
    fn open_service_args_accept_mobile_grant_file() {
        let cli = Cli::try_parse_from([
            "mobile-cli",
            "open-service",
            "--control",
            "http://127.0.0.1:4242",
            "--grant-file",
            "mobile-grant.json",
            "--client",
            "mobile_001",
            "--device",
            "pc_001",
            "--service",
            "svc_web_3000",
            "--local",
            "18080",
            "--relay-cert",
            "relay.der",
        ])
        .unwrap();

        let Commands::OpenService(args) = cli.command else {
            panic!("expected open-service command");
        };
        assert_eq!(
            args.grant_file.as_ref().unwrap().to_string_lossy(),
            "mobile-grant.json"
        );
        assert_eq!(args.token, None);
    }

    #[test]
    fn pair_args_accept_invite_and_grant_files() {
        let cli = Cli::try_parse_from([
            "mobile-cli",
            "pair",
            "--invite-file",
            "invite.json",
            "--grant-file",
            "grant.json",
            "--client",
            "mobile_001",
            "--service",
            "svc_web_3000",
            "--poll-ms",
            "250",
        ])
        .unwrap();

        let Commands::Pair(args) = cli.command else {
            panic!("expected pair command");
        };
        assert_eq!(args.invite_file.to_string_lossy(), "invite.json");
        assert_eq!(args.grant_file.to_string_lossy(), "grant.json");
        assert_eq!(args.client, "mobile_001");
        assert_eq!(args.services[0].as_str(), "svc_web_3000");
        assert_eq!(args.poll_ms, 250);
    }

    #[test]
    fn open_service_args_accept_direct_relay_mode() {
        let cli = Cli::try_parse_from([
            "mobile-cli",
            "open-service",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "user-token",
            "--client",
            "mobile_001",
            "--device",
            "pc_001",
            "--service",
            "svc_web_3000",
            "--local",
            "18080",
            "--relay-cert",
            "relay.der",
            "--session",
            "sess_001",
            "--relay-token",
            "relay-token",
            "--relay",
            "127.0.0.1:4443",
        ])
        .unwrap();

        let Commands::OpenService(args) = cli.command else {
            panic!("expected open-service command");
        };
        assert_eq!(args.relay_addr.unwrap().to_string(), "127.0.0.1:4443");
        assert_eq!(args.relay_cert.to_string_lossy(), "relay.der");
        assert_eq!(args.session_id.unwrap().as_str(), "sess_001");
        assert_eq!(args.relay_token.unwrap(), "relay-token");
    }

    #[test]
    fn open_service_args_build_sdk_p2p_or_relay_config_for_control_mode() {
        let cli = Cli::try_parse_from([
            "mobile-cli",
            "open-service",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "user-token",
            "--client",
            "mobile_001",
            "--device",
            "pc_001",
            "--service",
            "svc_web_3000",
            "--local",
            "18080",
            "--relay-cert",
            "relay.der",
            "--p2p-bind",
            "0.0.0.0:0",
            "--p2p-candidate-timeout-ms",
            "1500",
            "--p2p-probe-timeout-ms",
            "1500",
            "--p2p-interval-ms",
            "25",
            "--relay-fallback-delay-ms",
            "300",
        ])
        .unwrap();

        let Commands::OpenService(args) = cli.command else {
            panic!("expected open-service command");
        };
        let config = args.sdk_p2p_or_relay_config(CertificateDer::from(vec![1, 2, 3]));

        assert_eq!(config.bind_addr.to_string(), "0.0.0.0:0");
        assert_eq!(config.candidate_timeout.as_millis(), 1500);
        assert_eq!(config.probe_timeout.as_millis(), 1500);
        assert_eq!(config.interval.as_millis(), 25);
        assert_eq!(config.relay_fallback_delay.as_millis(), 300);
    }

    #[test]
    fn admin_users_args_build_admin_query() {
        let cli = Cli::try_parse_from([
            "mobile-cli",
            "admin",
            "users",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "admin-token",
            "--limit",
            "50",
            "--offset",
            "10",
            "--q",
            "alice",
            "--sort",
            "email",
            "--role",
            "admin",
            "--enabled",
            "true",
        ])
        .unwrap();

        let Commands::Admin(args) = cli.command else {
            panic!("expected admin command");
        };
        let AdminCommand::Users(query_args) = args.command else {
            panic!("expected users admin command");
        };
        assert_eq!(query_args.common.control, "http://127.0.0.1:4242");
        assert_eq!(query_args.common.token, "admin-token");
        let query = query_args.query.to_query();

        assert_eq!(query.limit, Some(50));
        assert_eq!(query.offset, Some(10));
        assert_eq!(query.q.as_deref(), Some("alice"));
        assert_eq!(query.sort.as_deref(), Some("email"));
        assert_eq!(query.role.as_deref(), Some("admin"));
        assert_eq!(query.enabled, Some(true));
    }

    #[test]
    fn admin_device_access_args_build_path_device_and_query() {
        let cli = Cli::try_parse_from([
            "mobile-cli",
            "admin",
            "device-access",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "admin-token",
            "--device",
            "server_001",
            "--user-id",
            "user_abc",
            "--limit",
            "20",
        ])
        .unwrap();

        let Commands::Admin(args) = cli.command else {
            panic!("expected admin command");
        };
        let AdminCommand::DeviceAccess(device_args) = args.command else {
            panic!("expected device-access admin command");
        };
        let query = device_args.query.to_query();

        assert_eq!(device_args.common.control, "http://127.0.0.1:4242");
        assert_eq!(device_args.common.token, "admin-token");
        assert_eq!(device_args.device.as_str(), "server_001");
        assert_eq!(query.user_id.unwrap().as_str(), "user_abc");
        assert_eq!(query.limit, Some(20));
    }

    #[test]
    fn admin_create_user_args_build_request() {
        let cli = Cli::try_parse_from([
            "mobile-cli",
            "admin",
            "create-user",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "admin-token",
            "--email",
            "member@example.com",
            "--password",
            "password-123",
            "--name",
            "Member",
            "--role",
            "admin",
            "--enabled",
            "false",
        ])
        .unwrap();

        let Commands::Admin(args) = cli.command else {
            panic!("expected admin command");
        };
        let AdminCommand::CreateUser(user_args) = args.command else {
            panic!("expected create-user command");
        };
        let request = user_args.request();

        assert_eq!(user_args.common.control, "http://127.0.0.1:4242");
        assert_eq!(request.email, "member@example.com");
        assert_eq!(request.display_name, "Member");
        assert_eq!(request.role, quic_tunnel_auth::ControlRole::Admin);
        assert!(!request.enabled);
    }

    #[test]
    fn admin_user_status_and_assign_plan_args_build_requests() {
        let status_cli = Cli::try_parse_from([
            "mobile-cli",
            "admin",
            "set-user-status",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "admin-token",
            "--user-id",
            "user_abc",
            "--enabled",
            "false",
        ])
        .unwrap();
        let Commands::Admin(status_args) = status_cli.command else {
            panic!("expected admin command");
        };
        let AdminCommand::SetUserStatus(status_args) = status_args.command else {
            panic!("expected set-user-status command");
        };
        assert_eq!(status_args.user_id.as_str(), "user_abc");
        assert!(!status_args.request().enabled);

        let plan_cli = Cli::try_parse_from([
            "mobile-cli",
            "admin",
            "assign-plan",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "admin-token",
            "--user-id",
            "user_abc",
            "--plan-id",
            "team",
        ])
        .unwrap();
        let Commands::Admin(plan_args) = plan_cli.command else {
            panic!("expected admin command");
        };
        let AdminCommand::AssignPlan(plan_args) = plan_args.command else {
            panic!("expected assign-plan command");
        };
        assert_eq!(plan_args.user_id.as_str(), "user_abc");
        assert_eq!(plan_args.request().plan_id, "team");
    }

    #[test]
    fn admin_relay_and_device_access_mutation_args_build_requests() {
        let relay_cli = Cli::try_parse_from([
            "mobile-cli",
            "admin",
            "register-relay",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "admin-token",
            "--relay-id",
            "relay_local",
            "--relay-addr",
            "127.0.0.1:4443",
            "--admin-addr",
            "127.0.0.1:9090",
            "--capacity-streams",
            "64",
        ])
        .unwrap();
        let Commands::Admin(relay_args) = relay_cli.command else {
            panic!("expected admin command");
        };
        let AdminCommand::RegisterRelay(relay_args) = relay_args.command else {
            panic!("expected register-relay command");
        };
        let request = relay_args.request();
        assert_eq!(request.relay_id, "relay_local");
        assert_eq!(request.capacity_streams, 64);

        let grant_cli = Cli::try_parse_from([
            "mobile-cli",
            "admin",
            "grant-device-access",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "admin-token",
            "--device",
            "server_001",
            "--user-id",
            "user_abc",
        ])
        .unwrap();
        let Commands::Admin(grant_args) = grant_cli.command else {
            panic!("expected admin command");
        };
        let AdminCommand::GrantDeviceAccess(grant_args) = grant_args.command else {
            panic!("expected grant-device-access command");
        };
        assert_eq!(grant_args.device.as_str(), "server_001");
        assert_eq!(grant_args.request().user_id.as_str(), "user_abc");
    }

    #[test]
    fn admin_create_relay_bootstrap_args_build_request() {
        let cli = Cli::try_parse_from([
            "mobile-cli",
            "admin",
            "create-relay-bootstrap",
            "--control",
            "https://control.example.com",
            "--token",
            "admin-token",
            "--relay-id",
            "relay_bootstrap",
            "--relay-addr",
            "relay.example.com:4443",
            "--admin-addr",
            "127.0.0.1:9090",
            "--capacity-streams",
            "128",
            "--heartbeat-interval-sec",
            "30",
            "--ttl-sec",
            "900",
        ])
        .unwrap();
        let Commands::Admin(args) = cli.command else {
            panic!("expected admin command");
        };
        let AdminCommand::CreateRelayBootstrap(args) = args.command else {
            panic!("expected create-relay-bootstrap command");
        };
        let request = args.request();
        assert_eq!(request.relay_id, "relay_bootstrap");
        assert_eq!(request.control_url, "https://control.example.com");
        assert_eq!(request.relay_addr, "relay.example.com:4443");
        assert_eq!(request.admin_addr, "127.0.0.1:9090");
        assert_eq!(request.capacity_streams, 128);
        assert_eq!(request.heartbeat_interval_sec, 30);
        assert_eq!(request.ttl_sec, 900);
    }

    #[test]
    fn admin_update_relay_and_revoke_device_access_args_build_requests() {
        let relay_cli = Cli::try_parse_from([
            "mobile-cli",
            "admin",
            "update-relay",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "admin-token",
            "--relay-id",
            "relay_local",
            "--relay-addr",
            "127.0.0.1:4444",
            "--admin-addr",
            "127.0.0.1:9091",
            "--capacity-streams",
            "32",
            "--healthy",
            "false",
        ])
        .unwrap();
        let Commands::Admin(relay_args) = relay_cli.command else {
            panic!("expected admin command");
        };
        let AdminCommand::UpdateRelay(relay_args) = relay_args.command else {
            panic!("expected update-relay command");
        };
        let request = relay_args.request();
        assert_eq!(relay_args.relay_id, "relay_local");
        assert_eq!(request.relay_addr, "127.0.0.1:4444");
        assert_eq!(request.capacity_streams, 32);
        assert!(!request.healthy);

        let revoke_cli = Cli::try_parse_from([
            "mobile-cli",
            "admin",
            "revoke-device-access",
            "--control",
            "http://127.0.0.1:4242",
            "--token",
            "admin-token",
            "--device",
            "server_001",
            "--user-id",
            "user_abc",
        ])
        .unwrap();
        let Commands::Admin(revoke_args) = revoke_cli.command else {
            panic!("expected admin command");
        };
        let AdminCommand::RevokeDeviceAccess(revoke_args) = revoke_args.command else {
            panic!("expected revoke-device-access command");
        };
        assert_eq!(revoke_args.device.as_str(), "server_001");
        assert_eq!(revoke_args.user_id.as_str(), "user_abc");
    }
}
