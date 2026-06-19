use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use mobilecode_connect_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, Service, ServiceId, ServiceProtocol,
};
use mobilecode_connect_sdk::{
    CreateSessionInput, EnsureBrowserServerLogin, EnsureDeviceCodeServerLogin, LoginInput,
    MobileCodeConnectSdk, RegisterControllerInput, RegisterInput, ServerLoginInput,
    ServerRegistrationInput, StoredToken,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let control_url = env_or(
        "MOBILECODE_CONNECT_CONTROL_URL",
        "http://127.0.0.1:8080".to_string(),
    );
    let state_dir = PathBuf::from(env_or(
        "MOBILECODE_CONNECT_SDK_STATE_DIR",
        "target/sdk-live-workflow".to_string(),
    ));
    let sdk = MobileCodeConnectSdk::builder()
        .control_url(control_url.clone())
        .token_file(state_dir.join("user-token.json"))
        .server_credential_file(state_dir.join("server-credential.json"))
        .build()?;

    println!("control_url={control_url}");
    println!("state_dir={}", state_dir.display());

    if env_flag("MOBILECODE_CONNECT_SDK_LIVE_RUN") != Some(true) {
        print_dry_run_help();
        return Ok(());
    }

    let token = ensure_user_token(&sdk).await?;
    let user_id = token.user_id.clone();

    let client_id = ClientId::new(env_or(
        "MOBILECODE_CONNECT_SDK_CLIENT_ID",
        "phone_001".to_string(),
    ));
    let device_id = DeviceId::new(env_or(
        "MOBILECODE_CONNECT_SDK_DEVICE_ID",
        "pc_001".to_string(),
    ));
    let device_name = env_or(
        "MOBILECODE_CONNECT_SDK_DEVICE_NAME",
        "Office PC".to_string(),
    );
    let service_id = ServiceId::new(env_or(
        "MOBILECODE_CONNECT_SDK_SERVICE_ID",
        "svc_web".to_string(),
    ));
    let service_host = env_or(
        "MOBILECODE_CONNECT_SDK_SERVICE_HOST",
        "127.0.0.1".to_string(),
    );
    let service_port =
        env_or("MOBILECODE_CONNECT_SDK_SERVICE_PORT", "3000".to_string()).parse::<u16>()?;

    let controller = sdk
        .ensure_controller(RegisterControllerInput {
            client_id: client_id.clone(),
            name: env_or("MOBILECODE_CONNECT_SDK_CLIENT_NAME", "Phone".to_string()),
        })
        .await?;
    println!("registered controller={}", controller.client_id);

    ensure_server_credential(&sdk, device_id.clone(), device_name.clone()).await?;

    sdk.server()?
        .register_server(ServerRegistrationInput {
            device: Device {
                device_id: device_id.clone(),
                user_id,
                name: device_name,
                status: DeviceStatus::Online,
                agent_version: "sdk-live-workflow".to_string(),
            },
            services: vec![Service {
                service_id: service_id.clone(),
                device_id: device_id.clone(),
                name: env_or(
                    "MOBILECODE_CONNECT_SDK_SERVICE_NAME",
                    service_id.to_string(),
                ),
                protocol: ServiceProtocol::Tcp,
                target_host: service_host,
                target_port: service_port,
            }],
            p2p_certificate_der: None,
        })
        .await?;
    println!("registered server device={device_id} service={service_id}");

    let devices = sdk.list_devices().await?;
    println!("visible devices={}", devices.len());
    let services = sdk.list_device_services(&device_id).await?;
    println!("device services={}", services.len());

    let session = sdk
        .create_session(CreateSessionInput {
            client_id,
            device_id,
            service_id,
        })
        .await?;
    println!(
        "created session={} relay={} punch={}",
        session.session_id, session.relay_addr, session.punch_addr
    );

    Ok(())
}

async fn ensure_user_token(
    sdk: &MobileCodeConnectSdk,
) -> Result<StoredToken, Box<dyn std::error::Error>> {
    let now_epoch_sec = now_epoch_sec()?;
    if let Some(token) = sdk.current_valid_token(now_epoch_sec).await? {
        println!("using saved user token for user={}", token.user_id);
        return Ok(token);
    }

    let email = required_env("MOBILECODE_CONNECT_SDK_EMAIL")?;
    let password = required_env("MOBILECODE_CONNECT_SDK_PASSWORD")?;
    let token = if env_flag("MOBILECODE_CONNECT_SDK_REGISTER") == Some(true) {
        sdk.ensure_register_fresh(
            RegisterInput {
                email,
                password,
                display_name: env_or(
                    "MOBILECODE_CONNECT_SDK_DISPLAY_NAME",
                    "SDK Example User".to_string(),
                ),
            },
            now_epoch_sec,
        )
        .await?
    } else {
        sdk.ensure_login_fresh(LoginInput { email, password }, now_epoch_sec)
            .await?
    };
    println!("using user token for user={}", token.user_id);
    Ok(token)
}

async fn ensure_server_credential(
    sdk: &MobileCodeConnectSdk,
    device_id: DeviceId,
    device_name: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let input = ServerLoginInput::existing_device(
        device_id,
        device_name,
        env_or(
            "MOBILECODE_CONNECT_SDK_SERVER_PUBLIC_KEY",
            "sdk-live-workflow-public-key".to_string(),
        ),
    );
    let server_auth = sdk.server_auth()?;

    if env_flag("MOBILECODE_CONNECT_SDK_BROWSER_SERVER_LOGIN") == Some(true) {
        return match sdk.ensure_browser_server_login(input).await? {
            EnsureBrowserServerLogin::Existing(credential) => {
                println!(
                    "using saved server credential={} device={}",
                    credential.credential_id, credential.device_id
                );
                Ok(())
            }
            EnsureBrowserServerLogin::Pending(pending) => {
                println!("open server auth url: {}", pending.auth_url);
                let auth_code = match env_var("MOBILECODE_CONNECT_SDK_SERVER_AUTH_CODE") {
                    Some(code) => code,
                    None => prompt("Server auth code: ")?,
                };
                let credential = server_auth
                    .complete_browser_login(pending, auth_code)
                    .await?;
                println!("saved server credential={}", credential.credential_id);
                Ok(())
            }
        };
    }

    match sdk.ensure_device_code_server_login(input).await? {
        EnsureDeviceCodeServerLogin::Existing(credential) => {
            println!(
                "using saved server credential={} device={}",
                credential.credential_id, credential.device_id
            );
            Ok(())
        }
        EnsureDeviceCodeServerLogin::Pending(pending) => {
            println!("open server auth url: {}", pending.verification_uri);
            println!("user code: {}", pending.user_code);
            println!("complete url: {}", pending.verification_uri_complete);
            let credential = server_auth
                .complete_device_code_login(pending, Duration::from_secs(1))
                .await?;
            println!("saved server credential={}", credential.credential_id);
            Ok(())
        }
    }
}

fn now_epoch_sec() -> Result<u64, Box<dyn std::error::Error>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn env_var(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .or_else(|| legacy_env_name(name).and_then(|legacy| env::var(legacy).ok()))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn legacy_env_name(name: &str) -> Option<String> {
    name.strip_prefix("MOBILECODE_CONNECT")
        .map(|suffix| format!("QUIC_TUNNEL{suffix}"))
}

fn env_or(name: &str, fallback: String) -> String {
    env_var(name).unwrap_or(fallback)
}

fn required_env(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    env_var(name).ok_or_else(|| format!("{name} is required").into())
}

fn env_flag(name: &str) -> Option<bool> {
    env_var(name).map(|value| matches!(value.as_str(), "1" | "true" | "yes"))
}

fn prompt(label: &str) -> Result<String, Box<dyn std::error::Error>> {
    print!("{label}");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err("input must not be empty".into());
    }
    Ok(value)
}

fn print_dry_run_help() {
    println!("dry_run=true");
    println!("set MOBILECODE_CONNECT_SDK_LIVE_RUN=1 to call the control server");
    println!("required for first login: MOBILECODE_CONNECT_SDK_EMAIL and MOBILECODE_CONNECT_SDK_PASSWORD");
    println!("set MOBILECODE_CONNECT_SDK_REGISTER=1 to create the user before saving the token");
    println!("device-code server login is used by default");
    println!("set MOBILECODE_CONNECT_SDK_BROWSER_SERVER_LOGIN=1 to use browser server login");
}
