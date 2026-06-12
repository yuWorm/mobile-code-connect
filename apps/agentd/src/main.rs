use std::{
    io::{self, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use mobilecode_connect_agent::{
    config::{AgentConfig, ServiceConfig},
    mobile_grant::{CreateMobileInviteRequest, MobileGrantManager},
    relay_client::{RelayAgentClient, RelayAgentConfig},
    runtime::{Agent, AgentControlRuntime, AgentControlRuntimeConfig, AgentP2pRuntimeConfig},
    service_registry::ServiceRegistry,
};
use mobilecode_connect_protocol::{
    mobile_grant_certificate_fingerprint, DeviceId, MobileInvitePayload, ServiceId,
    ServiceProtocol, SessionId,
};
use mobilecode_connect_sdk::server_auth::{
    FileServerCredentialStore, ServerAuthSdk, ServerCredentialStore, ServerLoginInput,
};
use mobilecode_connect_tunnel::quic::{generate_self_signed_server_identity, P2pQuicIdentity};
use rustls::pki_types::CertificateDer;

#[derive(Debug, Parser)]
#[command(name = "agentd")]
#[command(about = "Standalone PC Agent for MobileCode Connect")]
struct Cli {
    #[command(subcommand)]
    command: Option<AgentdCommand>,
    #[command(flatten)]
    run: RunArgs,
}

#[derive(Debug, Subcommand)]
enum AgentdCommand {
    Run(RunArgs),
    Login(LoginArgs),
    MobileInvite(MobileInviteArgs),
    MobileGrant(MobileGrantArgs),
}

#[derive(Debug, Clone, Args)]
struct RunArgs {
    #[arg(long = "relay")]
    relay_addr: Option<SocketAddr>,
    #[arg(long = "relay-cert")]
    relay_cert: Option<PathBuf>,
    #[arg(long = "session", value_parser = parse_session_id)]
    session_id: Option<SessionId>,
    #[arg(long = "relay-token")]
    relay_token: Option<String>,
    #[arg(long = "service", value_parser = parse_service_arg)]
    services: Vec<ServiceArg>,
    #[arg(long = "control")]
    control_server: Option<String>,
    #[arg(long = "device", value_parser = parse_device_id, default_value = "pc_001")]
    device_id: DeviceId,
    #[arg(long = "credential-file")]
    credential_file: Option<PathBuf>,
    #[arg(long = "agent-token", default_value = "agent-token")]
    agent_token: String,
    #[arg(long = "poll-ms", default_value_t = 1000)]
    poll_ms: u64,
    #[arg(long = "p2p-identity-dir")]
    p2p_identity_dir: Option<PathBuf>,
    #[arg(long = "p2p-bind", default_value = "0.0.0.0:0")]
    p2p_bind_addr: SocketAddr,
    #[arg(long = "p2p-candidate-timeout-ms", default_value_t = 1500)]
    p2p_candidate_timeout_ms: u64,
    #[arg(long = "p2p-probe-timeout-ms", default_value_t = 1500)]
    p2p_probe_timeout_ms: u64,
    #[arg(long = "p2p-interval-ms", default_value_t = 25)]
    p2p_interval_ms: u64,
    #[arg(long = "mobile-invite-service", value_parser = parse_service_id)]
    mobile_invite_services: Vec<ServiceId>,
    #[arg(long = "mobile-invite-ttl-sec", default_value_t = 600)]
    mobile_invite_ttl_sec: u64,
    #[arg(long = "mobile-invite-max-uses", default_value_t = 1)]
    mobile_invite_max_uses: u32,
    #[arg(long = "mobile-grants-file")]
    mobile_grants_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
struct LoginArgs {
    #[arg(long = "device-code")]
    device_code: bool,
    #[arg(long = "control")]
    control_server: String,
    #[arg(long = "device", value_parser = parse_device_id, default_value = "pc_001")]
    device_id: DeviceId,
    #[arg(long = "name")]
    device_name: String,
    #[arg(long = "credential-file", default_value = "agentd-credential.json")]
    credential_file: PathBuf,
    #[arg(long = "server-public-key")]
    server_public_key: Option<String>,
    #[arg(long = "poll-ms", default_value_t = 1000)]
    poll_ms: u64,
}

#[derive(Debug, Clone, Args)]
struct MobileInviteArgs {
    #[command(subcommand)]
    command: MobileInviteCommand,
}

#[derive(Debug, Clone, Subcommand)]
enum MobileInviteCommand {
    Create(CreateMobileInviteArgs),
    List(MobileGrantFileArgs),
    Revoke(RevokeMobileInviteArgs),
}

#[derive(Debug, Clone, Args)]
struct CreateMobileInviteArgs {
    #[arg(long = "mobile-grants-file")]
    mobile_grants_file: PathBuf,
    #[arg(long = "control")]
    control_server: String,
    #[arg(long = "device", value_parser = parse_device_id)]
    device_id: DeviceId,
    #[arg(long = "service", value_parser = parse_service_id)]
    services: Vec<ServiceId>,
    #[arg(long = "ttl-sec", default_value_t = 600)]
    ttl_sec: u64,
    #[arg(long = "max-uses", default_value_t = 1)]
    max_uses: u32,
    #[arg(long = "p2p-identity-dir")]
    p2p_identity_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
struct RevokeMobileInviteArgs {
    #[arg(long = "mobile-grants-file")]
    mobile_grants_file: PathBuf,
    #[arg(long = "invite-id")]
    invite_id: String,
}

#[derive(Debug, Clone, Args)]
struct MobileGrantArgs {
    #[command(subcommand)]
    command: MobileGrantCommand,
}

#[derive(Debug, Clone, Subcommand)]
enum MobileGrantCommand {
    List(MobileGrantFileArgs),
    Revoke(RevokeMobileGrantArgs),
}

#[derive(Debug, Clone, Args)]
struct RevokeMobileGrantArgs {
    #[arg(long = "mobile-grants-file")]
    mobile_grants_file: PathBuf,
    #[arg(long = "grant-id")]
    grant_id: String,
}

#[derive(Debug, Clone, Args)]
struct MobileGrantFileArgs {
    #[arg(long = "mobile-grants-file")]
    mobile_grants_file: PathBuf,
}

impl RunArgs {
    fn service_configs(&self) -> Vec<ServiceConfig> {
        self.services
            .iter()
            .map(|service| service.0.clone())
            .collect()
    }

    fn direct_relay_config(&self) -> Result<(SocketAddr, SessionId, String)> {
        match (&self.relay_addr, &self.session_id, &self.relay_token) {
            (Some(relay_addr), Some(session_id), Some(relay_token)) => {
                Ok((*relay_addr, session_id.clone(), relay_token.clone()))
            }
            (None, None, None) => anyhow::bail!(
                "--control is required unless --relay, --session, and --relay-token are provided"
            ),
            _ => anyhow::bail!(
                "--relay, --session, and --relay-token must be provided together for direct Relay mode"
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct ServiceArg(ServiceConfig);

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    match cli.command.unwrap_or(AgentdCommand::Run(cli.run)) {
        AgentdCommand::Run(args) => run_agent(args).await,
        AgentdCommand::Login(args) => login_agent(args).await,
        AgentdCommand::MobileInvite(args) => run_mobile_invite_command(args).await,
        AgentdCommand::MobileGrant(args) => run_mobile_grant_command(args).await,
    }
}

async fn run_agent(mut cli: RunArgs) -> Result<()> {
    if let Some(credential_file) = &cli.credential_file {
        let store = FileServerCredentialStore::new(credential_file.clone());
        let credential = store
            .load_credential()
            .await
            .with_context(|| format!("read agent credential {}", credential_file.display()))?
            .with_context(|| {
                format!(
                    "agent credential {} does not exist",
                    credential_file.display()
                )
            })?;
        if cli.control_server.is_none() {
            cli.control_server = Some(credential.control_server.clone());
        }
        cli.device_id = credential.device_id;
        cli.agent_token = credential.server_token;
    }

    let relay_cert = cli
        .relay_cert
        .as_ref()
        .context("--relay-cert is required for agentd run")?;
    let server_cert = CertificateDer::from(
        tokio::fs::read(relay_cert)
            .await
            .with_context(|| format!("read relay certificate {}", relay_cert.display()))?,
    );
    let registry = ServiceRegistry::new(cli.service_configs()).context("build service registry")?;
    if let Some(control_server) = &cli.control_server {
        let p2p_identity = if let Some(identity_dir) = &cli.p2p_identity_dir {
            Some(
                load_or_generate_p2p_identity(identity_dir)
                    .await
                    .with_context(|| {
                        format!("load p2p identity from {}", identity_dir.display())
                    })?,
            )
        } else {
            None
        };
        Agent::register_with_control(AgentConfig {
            device_id: cli.device_id.clone(),
            control_server: control_server.clone(),
            auth_token: cli.agent_token.clone(),
            services: cli.service_configs(),
            p2p_certificate_der: p2p_identity
                .as_ref()
                .map(|identity| identity.certificate_der().as_ref().to_vec()),
        })
        .await
        .context("register agent with control")?;

        let agent_p2p_cert_fingerprint = p2p_identity.as_ref().map(|identity| {
            mobile_grant_certificate_fingerprint(identity.certificate_der().as_ref())
        });
        let p2p = p2p_identity.map(|identity| AgentP2pRuntimeConfig {
            bind_addr: cli.p2p_bind_addr,
            candidate_timeout: Duration::from_millis(cli.p2p_candidate_timeout_ms),
            probe_timeout: Duration::from_millis(cli.p2p_probe_timeout_ms),
            interval: Duration::from_millis(cli.p2p_interval_ms),
            server_identity: Some(identity),
        });
        let mobile_grants = if let Some(grants_file) = &cli.mobile_grants_file {
            MobileGrantManager::load_or_create_file(grants_file)
                .with_context(|| format!("load mobile grants file {}", grants_file.display()))?
        } else {
            MobileGrantManager::default()
        };
        if !cli.mobile_invite_services.is_empty() {
            for service_id in &cli.mobile_invite_services {
                if !cli
                    .services
                    .iter()
                    .any(|service| &service.0.service_id == service_id)
                {
                    anyhow::bail!(
                        "--mobile-invite-service {service_id} must match a registered --service"
                    );
                }
            }
            let invite = mobile_grants
                .create_invite(
                    CreateMobileInviteRequest {
                        control_url: control_server.clone(),
                        device_id: cli.device_id.clone(),
                        allowed_services: cli.mobile_invite_services.clone(),
                        ttl_sec: cli.mobile_invite_ttl_sec,
                        max_uses: cli.mobile_invite_max_uses,
                        agent_p2p_cert_fingerprint: agent_p2p_cert_fingerprint.clone(),
                    },
                    current_epoch_sec(),
                )
                .context("create mobile invite")?;
            println!(
                "mobile invite: {}",
                serde_json::to_string(&invite).context("serialize mobile invite")?
            );
        }
        let agent = AgentControlRuntime::new(AgentControlRuntimeConfig {
            control_server_url: control_server.clone(),
            auth_token: cli.agent_token.clone(),
            device_id: cli.device_id.clone(),
            relay_server_cert: server_cert,
            registry,
            poll_interval: Duration::from_millis(cli.poll_ms),
            p2p,
            mobile_grants: Some(mobile_grants),
        })
        .context("build agent control runtime")?;

        println!("agentd polling control {control_server}");
        agent
            .run_until(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        return Ok(());
    }

    let (relay_addr, session_id, relay_token) = cli.direct_relay_config()?;
    let agent = RelayAgentClient::connect(RelayAgentConfig {
        relay_addr,
        server_cert,
        session_id,
        token: relay_token,
        registry,
    })
    .await
    .context("connect agent to relay")?;

    println!("agentd connected to relay {relay_addr}");
    agent
        .run_until(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

async fn run_mobile_invite_command(args: MobileInviteArgs) -> Result<()> {
    match args.command {
        MobileInviteCommand::Create(args) => {
            let invite = create_mobile_invite(args).await?;
            println!("{}", serde_json::to_string_pretty(&invite)?);
        }
        MobileInviteCommand::List(args) => {
            let manager = mobile_grant_manager_from_file(&args.mobile_grants_file)?;
            println!("{}", serde_json::to_string_pretty(&manager.list_invites())?);
        }
        MobileInviteCommand::Revoke(args) => {
            let manager = mobile_grant_manager_from_file(&args.mobile_grants_file)?;
            manager.revoke_invite(&args.invite_id)?;
            println!("revoked mobile invite {}", args.invite_id);
        }
    }
    Ok(())
}

async fn run_mobile_grant_command(args: MobileGrantArgs) -> Result<()> {
    match args.command {
        MobileGrantCommand::List(args) => {
            let manager = mobile_grant_manager_from_file(&args.mobile_grants_file)?;
            println!("{}", serde_json::to_string_pretty(&manager.list_grants())?);
        }
        MobileGrantCommand::Revoke(args) => {
            let manager = mobile_grant_manager_from_file(&args.mobile_grants_file)?;
            manager.revoke_grant(&args.grant_id)?;
            println!("revoked mobile grant {}", args.grant_id);
        }
    }
    Ok(())
}

async fn create_mobile_invite(args: CreateMobileInviteArgs) -> Result<MobileInvitePayload> {
    if args.services.is_empty() {
        anyhow::bail!("at least one --service is required");
    }
    let agent_p2p_cert_fingerprint = if let Some(identity_dir) = &args.p2p_identity_dir {
        let identity = load_or_generate_p2p_identity(identity_dir)
            .await
            .with_context(|| format!("load p2p identity from {}", identity_dir.display()))?;
        Some(mobile_grant_certificate_fingerprint(
            identity.certificate_der().as_ref(),
        ))
    } else {
        None
    };
    let manager = mobile_grant_manager_from_file(&args.mobile_grants_file)?;
    manager
        .create_invite(
            CreateMobileInviteRequest {
                control_url: args.control_server,
                device_id: args.device_id,
                allowed_services: args.services,
                ttl_sec: args.ttl_sec,
                max_uses: args.max_uses,
                agent_p2p_cert_fingerprint,
            },
            current_epoch_sec(),
        )
        .context("create mobile invite")
}

fn mobile_grant_manager_from_file(path: &Path) -> Result<MobileGrantManager> {
    MobileGrantManager::load_or_create_file(path)
        .with_context(|| format!("load mobile grants file {}", path.display()))
}

async fn login_agent(args: LoginArgs) -> Result<()> {
    let server_public_key = args
        .server_public_key
        .clone()
        .unwrap_or_else(|| format!("agentd-public-key-{}", args.device_id));
    let sdk = ServerAuthSdk::with_http_client(
        &args.control_server,
        FileServerCredentialStore::new(args.credential_file.clone()),
    )
    .context("build server auth sdk")?;
    let input = ServerLoginInput {
        device_id: args.device_id.clone(),
        device_name: args.device_name.clone(),
        server_public_key,
    };

    if args.device_code {
        let start = sdk
            .start_device_code_login(input)
            .await
            .context("start device-code server auth")?;
        println!("Open: {}", start.verification_uri);
        println!("Code: {}", start.user_code);
        println!("Complete URL: {}", start.verification_uri_complete);
        sdk.complete_device_code_login(start, Duration::from_millis(args.poll_ms))
            .await
            .context("poll device-code server auth")?;
    } else {
        let start = sdk
            .start_browser_login(input)
            .await
            .context("start browser server auth")?;
        println!("Open: {}", start.auth_url);
        let server_auth_code = prompt_server_auth_code()?;
        sdk.complete_browser_login(start, server_auth_code)
            .await
            .context("exchange browser server auth")?;
    }
    println!("saved agent credential {}", args.credential_file.display());
    Ok(())
}

fn prompt_server_auth_code() -> Result<String> {
    print!("Server auth code: ");
    io::stdout().flush().context("flush prompt")?;
    let mut code = String::new();
    io::stdin()
        .read_line(&mut code)
        .context("read server auth code")?;
    let code = code.trim().to_string();
    if code.is_empty() {
        anyhow::bail!("server auth code must not be empty");
    }
    Ok(code)
}

async fn load_or_generate_p2p_identity(identity_dir: &Path) -> Result<P2pQuicIdentity> {
    let cert_path = identity_dir.join("cert.der");
    let key_path = identity_dir.join("key.pkcs8.der");

    match (
        tokio::fs::try_exists(&cert_path)
            .await
            .with_context(|| format!("check p2p certificate path {}", cert_path.display()))?,
        tokio::fs::try_exists(&key_path)
            .await
            .with_context(|| format!("check p2p private key path {}", key_path.display()))?,
    ) {
        (true, true) => {
            let certificate_der = tokio::fs::read(&cert_path)
                .await
                .with_context(|| format!("read p2p certificate {}", cert_path.display()))?;
            let private_key_der = tokio::fs::read(&key_path)
                .await
                .with_context(|| format!("read p2p private key {}", key_path.display()))?;
            Ok(P2pQuicIdentity::from_der_parts(
                certificate_der,
                private_key_der,
            ))
        }
        (false, false) => {
            tokio::fs::create_dir_all(identity_dir)
                .await
                .with_context(|| format!("create p2p identity dir {}", identity_dir.display()))?;
            let identity =
                generate_self_signed_server_identity().context("generate p2p identity")?;
            tokio::fs::write(&cert_path, identity.certificate_der().as_ref())
                .await
                .with_context(|| format!("write p2p certificate {}", cert_path.display()))?;
            write_private_key(&key_path, identity.private_key_der()).await?;
            Ok(identity)
        }
        (true, false) => anyhow::bail!(
            "p2p identity dir {} has cert.der but is missing key.pkcs8.der",
            identity_dir.display()
        ),
        (false, true) => anyhow::bail!(
            "p2p identity dir {} has key.pkcs8.der but is missing cert.der",
            identity_dir.display()
        ),
    }
}

async fn write_private_key(path: &Path, private_key_der: &[u8]) -> Result<()> {
    tokio::fs::write(path, private_key_der)
        .await
        .with_context(|| format!("write p2p private key {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .await
            .with_context(|| format!("set p2p private key permissions {}", path.display()))?;
    }

    Ok(())
}

fn parse_session_id(value: &str) -> std::result::Result<SessionId, String> {
    if value.trim().is_empty() {
        return Err("session id must not be empty".to_string());
    }
    Ok(SessionId::new(value))
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

fn parse_service_arg(value: &str) -> std::result::Result<ServiceArg, String> {
    let (service_id, target) = value
        .split_once('=')
        .ok_or_else(|| "service must use service_id=host:port".to_string())?;
    if service_id.trim().is_empty() {
        return Err("service id must not be empty".to_string());
    }

    let (host, port) = target
        .rsplit_once(':')
        .ok_or_else(|| "service target must use host:port".to_string())?;
    if host.trim().is_empty() {
        return Err("service host must not be empty".to_string());
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| "service port must be a valid u16".to_string())?;

    Ok(ServiceArg(ServiceConfig {
        service_id: ServiceId::new(service_id),
        name: service_id.to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: host.to_string(),
        target_port: port,
    }))
}

fn current_epoch_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
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
    fn agentd_args_build_direct_relay_config_and_service_registry_input() {
        let cli = Cli::try_parse_from([
            "agentd",
            "--relay",
            "127.0.0.1:4443",
            "--relay-cert",
            "relay.der",
            "--session",
            "sess_001",
            "--relay-token",
            "relay-token",
            "--service",
            "svc_web_3000=127.0.0.1:3000",
        ])
        .unwrap();

        assert_eq!(
            cli.run.relay_addr.as_ref().unwrap().to_string(),
            "127.0.0.1:4443"
        );
        assert_eq!(
            cli.run.relay_cert.as_ref().unwrap().to_string_lossy(),
            "relay.der"
        );
        assert_eq!(cli.run.session_id.as_ref().unwrap().as_str(), "sess_001");
        assert_eq!(cli.run.relay_token.as_deref(), Some("relay-token"));
        assert_eq!(cli.run.control_server, None);

        let services = cli.run.service_configs();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].service_id.as_str(), "svc_web_3000");
        assert_eq!(services[0].target_host, "127.0.0.1");
        assert_eq!(services[0].target_port, 3000);
    }

    #[test]
    fn agentd_args_accept_control_poll_mode_without_manual_session_token() {
        let cli = Cli::try_parse_from([
            "agentd",
            "--relay-cert",
            "relay.der",
            "--service",
            "svc_web_3000=127.0.0.1:3000",
            "--control",
            "http://127.0.0.1:4242",
            "--device",
            "pc_001",
            "--agent-token",
            "agent-token",
            "--poll-ms",
            "250",
        ])
        .unwrap();

        assert_eq!(cli.run.relay_addr, None);
        assert_eq!(cli.run.session_id, None);
        assert_eq!(cli.run.relay_token, None);
        assert_eq!(
            cli.run.control_server.as_deref(),
            Some("http://127.0.0.1:4242")
        );
        assert_eq!(cli.run.device_id.as_str(), "pc_001");
        assert_eq!(cli.run.agent_token, "agent-token");
        assert_eq!(cli.run.poll_ms, 250);
    }

    #[test]
    fn agentd_args_accept_mobile_invite_scope_for_control_mode() {
        let cli = Cli::try_parse_from([
            "agentd",
            "--relay-cert",
            "relay.der",
            "--service",
            "svc_web_3000=127.0.0.1:3000",
            "--control",
            "http://127.0.0.1:4242",
            "--mobile-invite-service",
            "svc_web_3000",
            "--mobile-invite-ttl-sec",
            "900",
            "--mobile-invite-max-uses",
            "2",
            "--mobile-grants-file",
            "agentd-mobile-grants.json",
        ])
        .unwrap();

        assert_eq!(cli.run.mobile_invite_services.len(), 1);
        assert_eq!(cli.run.mobile_invite_services[0].as_str(), "svc_web_3000");
        assert_eq!(cli.run.mobile_invite_ttl_sec, 900);
        assert_eq!(cli.run.mobile_invite_max_uses, 2);
        assert_eq!(
            cli.run
                .mobile_grants_file
                .as_ref()
                .unwrap()
                .to_string_lossy(),
            "agentd-mobile-grants.json"
        );
    }

    #[test]
    fn agentd_args_accept_p2p_identity_for_control_mode() {
        let cli = Cli::try_parse_from([
            "agentd",
            "--relay-cert",
            "relay.der",
            "--service",
            "svc_web_3000=127.0.0.1:3000",
            "--control",
            "http://127.0.0.1:4242",
            "--p2p-identity-dir",
            "agent-identity",
            "--p2p-bind",
            "0.0.0.0:0",
            "--p2p-candidate-timeout-ms",
            "1500",
            "--p2p-probe-timeout-ms",
            "1500",
            "--p2p-interval-ms",
            "25",
        ])
        .unwrap();

        assert_eq!(
            cli.run.p2p_identity_dir.as_ref().unwrap().to_string_lossy(),
            "agent-identity"
        );
        assert_eq!(cli.run.p2p_bind_addr.to_string(), "0.0.0.0:0");
        assert_eq!(cli.run.p2p_candidate_timeout_ms, 1500);
        assert_eq!(cli.run.p2p_probe_timeout_ms, 1500);
        assert_eq!(cli.run.p2p_interval_ms, 25);
    }

    #[test]
    fn login_args_accept_browser_and_device_code_modes() {
        let browser = Cli::try_parse_from([
            "agentd",
            "login",
            "--control",
            "http://127.0.0.1:4242",
            "--device",
            "pc_001",
            "--name",
            "Office PC",
            "--credential-file",
            "agentd-credential.json",
        ])
        .unwrap();
        match browser.command.unwrap() {
            AgentdCommand::Login(args) => {
                assert!(!args.device_code);
                assert_eq!(args.control_server, "http://127.0.0.1:4242");
                assert_eq!(args.device_id.as_str(), "pc_001");
                assert_eq!(args.device_name, "Office PC");
                assert_eq!(
                    args.credential_file.to_string_lossy(),
                    "agentd-credential.json"
                );
            }
            _ => panic!("expected login command"),
        }

        let device_code = Cli::try_parse_from([
            "agentd",
            "login",
            "--device-code",
            "--control",
            "http://127.0.0.1:4242",
            "--device",
            "pc_002",
            "--name",
            "Headless PC",
        ])
        .unwrap();
        match device_code.command.unwrap() {
            AgentdCommand::Login(args) => {
                assert!(args.device_code);
                assert_eq!(args.device_id.as_str(), "pc_002");
                assert_eq!(args.device_name, "Headless PC");
            }
            _ => panic!("expected login command"),
        }
    }

    #[test]
    fn run_args_accept_credential_file_subcommand() {
        let cli = Cli::try_parse_from([
            "agentd",
            "run",
            "--relay-cert",
            "relay.der",
            "--service",
            "svc_web_3000=127.0.0.1:3000",
            "--control",
            "http://127.0.0.1:4242",
            "--credential-file",
            "agentd-credential.json",
        ])
        .unwrap();

        match cli.command.unwrap() {
            AgentdCommand::Run(args) => {
                assert_eq!(
                    args.control_server.as_deref(),
                    Some("http://127.0.0.1:4242")
                );
                assert_eq!(
                    args.credential_file.as_ref().unwrap().to_string_lossy(),
                    "agentd-credential.json"
                );
            }
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn agentd_args_accept_mobile_invite_admin_commands() {
        let create = Cli::try_parse_from([
            "agentd",
            "mobile-invite",
            "create",
            "--mobile-grants-file",
            "agentd-mobile-grants.json",
            "--control",
            "http://127.0.0.1:4242",
            "--device",
            "pc_001",
            "--service",
            "svc_web_3000",
            "--ttl-sec",
            "900",
            "--max-uses",
            "2",
            "--p2p-identity-dir",
            "agent-identity",
        ])
        .unwrap();
        match create.command.unwrap() {
            AgentdCommand::MobileInvite(args) => match args.command {
                MobileInviteCommand::Create(args) => {
                    assert_eq!(
                        args.mobile_grants_file.to_string_lossy(),
                        "agentd-mobile-grants.json"
                    );
                    assert_eq!(args.control_server, "http://127.0.0.1:4242");
                    assert_eq!(args.device_id.as_str(), "pc_001");
                    assert_eq!(args.services[0].as_str(), "svc_web_3000");
                    assert_eq!(args.ttl_sec, 900);
                    assert_eq!(args.max_uses, 2);
                    assert_eq!(
                        args.p2p_identity_dir.as_ref().unwrap().to_string_lossy(),
                        "agent-identity"
                    );
                }
                _ => panic!("expected create"),
            },
            _ => panic!("expected mobile-invite command"),
        }

        let list = Cli::try_parse_from([
            "agentd",
            "mobile-invite",
            "list",
            "--mobile-grants-file",
            "agentd-mobile-grants.json",
        ])
        .unwrap();
        assert!(matches!(
            list.command.unwrap(),
            AgentdCommand::MobileInvite(MobileInviteArgs {
                command: MobileInviteCommand::List(_)
            })
        ));

        let revoke = Cli::try_parse_from([
            "agentd",
            "mobile-invite",
            "revoke",
            "--mobile-grants-file",
            "agentd-mobile-grants.json",
            "--invite-id",
            "inv_001",
        ])
        .unwrap();
        match revoke.command.unwrap() {
            AgentdCommand::MobileInvite(args) => match args.command {
                MobileInviteCommand::Revoke(args) => {
                    assert_eq!(args.invite_id, "inv_001");
                }
                _ => panic!("expected revoke"),
            },
            _ => panic!("expected mobile-invite command"),
        }
    }

    #[test]
    fn agentd_args_accept_mobile_grant_admin_commands() {
        let list = Cli::try_parse_from([
            "agentd",
            "mobile-grant",
            "list",
            "--mobile-grants-file",
            "agentd-mobile-grants.json",
        ])
        .unwrap();
        assert!(matches!(
            list.command.unwrap(),
            AgentdCommand::MobileGrant(MobileGrantArgs {
                command: MobileGrantCommand::List(_)
            })
        ));

        let revoke = Cli::try_parse_from([
            "agentd",
            "mobile-grant",
            "revoke",
            "--mobile-grants-file",
            "agentd-mobile-grants.json",
            "--grant-id",
            "gr_001",
        ])
        .unwrap();
        match revoke.command.unwrap() {
            AgentdCommand::MobileGrant(args) => match args.command {
                MobileGrantCommand::Revoke(args) => {
                    assert_eq!(args.grant_id, "gr_001");
                }
                _ => panic!("expected revoke"),
            },
            _ => panic!("expected mobile-grant command"),
        }
    }

    #[tokio::test]
    async fn mobile_invite_create_command_persists_p2p_fingerprint() {
        let grants_file = unique_temp_dir().join("mobile-grants.json");
        let identity_dir = unique_temp_dir();
        let invite = create_mobile_invite(CreateMobileInviteArgs {
            mobile_grants_file: grants_file.clone(),
            control_server: "http://127.0.0.1:4242".to_string(),
            device_id: DeviceId::new("pc_001"),
            services: vec![ServiceId::new("svc_web_3000")],
            ttl_sec: 900,
            max_uses: 2,
            p2p_identity_dir: Some(identity_dir.clone()),
        })
        .await
        .unwrap();

        assert!(invite.agent_p2p_cert_fingerprint.is_some());
        let manager = MobileGrantManager::load_or_create_file(&grants_file).unwrap();
        assert_eq!(
            manager.list_invites()[0].agent_p2p_cert_fingerprint,
            invite.agent_p2p_cert_fingerprint
        );

        tokio::fs::remove_file(grants_file).await.unwrap();
        tokio::fs::remove_dir_all(identity_dir).await.unwrap();
    }

    #[tokio::test]
    async fn p2p_identity_dir_is_generated_then_reused() {
        let dir = unique_temp_dir();
        let first = load_or_generate_p2p_identity(&dir).await.unwrap();
        let first_cert = first.certificate_der().as_ref().to_vec();

        assert!(dir.join("cert.der").is_file());
        assert!(dir.join("key.pkcs8.der").is_file());

        let second = load_or_generate_p2p_identity(&dir).await.unwrap();
        assert_eq!(second.certificate_der().as_ref(), first_cert.as_slice());

        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    fn unique_temp_dir() -> PathBuf {
        static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("mobilecode-connect-agentd-{suffix}-{id}"))
    }
}
