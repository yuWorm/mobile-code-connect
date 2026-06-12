use std::{net::SocketAddr, time::Duration};

use mobilecode_connect_control_client::{ControlClientError, StatusCode};
use mobilecode_connect_protocol::{ClientId, DeviceId, ServiceId, UserId};
use mobilecode_connect_sdk::{
    server_auth::ServerCredentialStore,
    store::{StoredToken, TokenStore},
    CreateSessionInput, EnsureBrowserServerLogin, EnsureDeviceCodeServerLogin,
    HttpControlClientOptions, LoginInput, MobileCodeConnectSdk, OpenMobileServiceInput,
    P2pOrRelayTunnelConfig, RegisterControllerInput, RegisterInput, SdkError, ServerLoginInput,
    StoredServerCredential,
};
use rustls::pki_types::CertificateDer;

#[tokio::test]
async fn builder_defaults_to_memory_token_store() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:4242")
        .build()
        .unwrap();

    assert_eq!(sdk.control_url(), "http://127.0.0.1:4242");
    assert_eq!(sdk.current_token().await.unwrap(), None);
}

#[tokio::test]
async fn builder_file_token_store_is_shared_across_facades() {
    let dir = unique_temp_dir();
    let token_path = dir.join("token.json");
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:4242")
        .token_file(token_path.clone())
        .build()
        .unwrap();
    let token = StoredToken {
        user_id: UserId::new("user_001"),
        access_token: "token.shared".to_string(),
        expire_at: 100,
    };

    sdk.token_store().save_token(token.clone()).await.unwrap();

    assert_eq!(sdk.current_token().await.unwrap(), Some(token.clone()));
    assert_eq!(
        sdk.auth().unwrap().current_token().await.unwrap(),
        Some(token.clone())
    );
    assert_eq!(
        sdk.controller().unwrap().current_token().await.unwrap(),
        Some(token)
    );
    assert!(token_path.is_file());

    tokio::fs::remove_dir_all(&dir).await.unwrap();
}

#[tokio::test]
async fn builder_file_server_credential_store_is_shared_across_server_facades() {
    let dir = unique_temp_dir();
    let credential_path = dir.join("server-credential.json");
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:4242")
        .server_credential_file(credential_path.clone())
        .build()
        .unwrap();
    let credential = server_credential();

    sdk.server_credential_store()
        .save_credential(credential.clone())
        .await
        .unwrap();

    assert_eq!(
        sdk.current_server_credential().await.unwrap(),
        Some(credential.clone())
    );
    assert_eq!(
        sdk.server_auth().unwrap().load_credential().await.unwrap(),
        Some(credential.clone())
    );
    assert_eq!(
        sdk.server().unwrap().load_credential().await.unwrap(),
        Some(credential)
    );
    assert!(credential_path.is_file());

    sdk.clear_server_credential().await.unwrap();
    assert_eq!(sdk.current_server_credential().await.unwrap(), None);

    tokio::fs::remove_dir_all(&dir).await.unwrap();
}

#[test]
fn builder_accepts_control_client_timeout_and_retry_options() {
    let options = HttpControlClientOptions::default()
        .with_request_timeout(Duration::from_secs(5))
        .with_max_retries(2)
        .with_retry_backoff(Duration::from_millis(25));

    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:4242")
        .control_client_options(options)
        .build()
        .unwrap();

    assert_eq!(sdk.control_client_options(), &options);
}

#[tokio::test]
async fn facade_admin_uses_shared_user_token_store() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:4242")
        .build()
        .unwrap();
    let token = StoredToken {
        user_id: UserId::new("admin_001"),
        access_token: "token.admin".to_string(),
        expire_at: 100,
    };

    sdk.token_store().save_token(token.clone()).await.unwrap();

    assert_eq!(
        sdk.admin().unwrap().current_token().await.unwrap(),
        Some(token)
    );
}

#[tokio::test]
async fn facade_ensure_auth_helpers_reuse_saved_token_without_control_request() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("https://127.0.0.1:4242")
        .build()
        .unwrap();
    let token = StoredToken {
        user_id: UserId::new("user_001"),
        access_token: "token.saved".to_string(),
        expire_at: 100,
    };
    sdk.token_store().save_token(token.clone()).await.unwrap();

    assert_eq!(
        sdk.ensure_login(LoginInput {
            email: "facade@example.com".to_string(),
            password: "password-123".to_string(),
        })
        .await
        .unwrap(),
        token
    );
    assert_eq!(
        sdk.ensure_register(RegisterInput {
            email: "facade@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Facade User".to_string(),
        })
        .await
        .unwrap()
        .access_token,
        "token.saved"
    );
}

#[tokio::test]
async fn facade_ensure_auth_helpers_login_or_register_when_token_is_missing() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("https://127.0.0.1:4242")
        .build()
        .unwrap();

    assert_control_config_error(
        sdk.ensure_login(LoginInput {
            email: "facade@example.com".to_string(),
            password: "password-123".to_string(),
        })
        .await
        .unwrap_err(),
    );
    assert_control_config_error(
        sdk.ensure_register(RegisterInput {
            email: "facade@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Facade User".to_string(),
        })
        .await
        .unwrap_err(),
    );
}

#[tokio::test]
async fn facade_current_valid_token_filters_expired_saved_token() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:4242")
        .build()
        .unwrap();
    let token = StoredToken {
        user_id: UserId::new("user_001"),
        access_token: "token.saved".to_string(),
        expire_at: 100,
    };
    sdk.token_store().save_token(token.clone()).await.unwrap();

    assert_eq!(sdk.current_valid_token(99).await.unwrap(), Some(token));
    assert_eq!(sdk.current_valid_token(100).await.unwrap(), None);
    assert_eq!(sdk.current_valid_token(101).await.unwrap(), None);
}

#[tokio::test]
async fn facade_ensure_auth_fresh_helpers_reuse_only_unexpired_saved_token() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("https://127.0.0.1:4242")
        .build()
        .unwrap();
    let token = StoredToken {
        user_id: UserId::new("user_001"),
        access_token: "token.saved".to_string(),
        expire_at: 100,
    };
    sdk.token_store().save_token(token.clone()).await.unwrap();

    assert_eq!(
        sdk.ensure_login_fresh(login_input(), 99).await.unwrap(),
        token.clone()
    );
    assert_eq!(
        sdk.ensure_register_fresh(register_input(), 99)
            .await
            .unwrap(),
        token
    );
    assert_control_config_error(
        sdk.ensure_login_fresh(login_input(), 100)
            .await
            .unwrap_err(),
    );
    assert_control_config_error(
        sdk.ensure_register_fresh(register_input(), 100)
            .await
            .unwrap_err(),
    );
}

#[tokio::test]
async fn facade_ensure_controller_requires_saved_token_before_control_request() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("https://127.0.0.1:4242")
        .build()
        .unwrap();

    let err = sdk
        .ensure_controller(RegisterControllerInput {
            client_id: ClientId::new("phone_001"),
            name: "Phone".to_string(),
        })
        .await
        .unwrap_err();

    assert!(matches!(err, SdkError::NotAuthenticated));
}

#[tokio::test]
async fn facade_ensure_server_login_helpers_reuse_saved_credential_without_control_request() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("https://127.0.0.1:4242")
        .build()
        .unwrap();
    let credential = server_credential_for("https://127.0.0.1:4242");
    sdk.server_credential_store()
        .save_credential(credential.clone())
        .await
        .unwrap();

    assert_eq!(
        sdk.ensure_browser_server_login(server_login_input())
            .await
            .unwrap(),
        EnsureBrowserServerLogin::Existing(credential.clone())
    );
    assert_eq!(
        sdk.ensure_device_code_server_login(server_login_input())
            .await
            .unwrap(),
        EnsureDeviceCodeServerLogin::Existing(credential)
    );
}

#[tokio::test]
async fn facade_ensure_server_login_helpers_ignore_credential_for_another_control_server() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("https://127.0.0.1:4242")
        .build()
        .unwrap();
    sdk.server_credential_store()
        .save_credential(server_credential_for("http://old-control.local:8080"))
        .await
        .unwrap();

    assert_eq!(
        sdk.current_server_credential_for_control().await.unwrap(),
        None
    );
    assert_control_config_error(
        sdk.ensure_browser_server_login(server_login_input())
            .await
            .unwrap_err(),
    );
    assert_control_config_error(
        sdk.ensure_device_code_server_login(server_login_input())
            .await
            .unwrap_err(),
    );
}

#[tokio::test]
async fn facade_ensure_server_login_helpers_start_login_when_credential_is_missing() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("https://127.0.0.1:4242")
        .build()
        .unwrap();

    assert_control_config_error(
        sdk.ensure_browser_server_login(server_login_input())
            .await
            .unwrap_err(),
    );
    assert_control_config_error(
        sdk.ensure_device_code_server_login(server_login_input())
            .await
            .unwrap_err(),
    );
}

#[tokio::test]
async fn facade_starts_mobile_tunnel_from_shared_token_store() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:4242")
        .build()
        .unwrap();
    sdk.token_store()
        .save_token(StoredToken {
            user_id: UserId::new("user_001"),
            access_token: "token.mobile".to_string(),
            expire_at: 100,
        })
        .await
        .unwrap();

    let tunnel = sdk
        .start_mobile_tunnel_in_memory(ClientId::new("phone_001"))
        .await
        .unwrap();

    assert_eq!(tunnel.status().active_forwards, 0);
}

#[tokio::test]
async fn facade_auth_controller_methods_surface_invalid_control_url() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("https://127.0.0.1:4242")
        .build()
        .unwrap();

    assert_control_config_error(
        sdk.register(RegisterInput {
            email: "facade@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Facade User".to_string(),
        })
        .await
        .unwrap_err(),
    );
    assert_control_config_error(
        sdk.login(LoginInput {
            email: "facade@example.com".to_string(),
            password: "password-123".to_string(),
        })
        .await
        .unwrap_err(),
    );
    assert_control_config_error(
        sdk.update_password(Some("password-123".to_string()), "password-456")
            .await
            .unwrap_err(),
    );
    assert_control_config_error(
        sdk.register_controller(RegisterControllerInput {
            client_id: ClientId::new("phone_001"),
            name: "Phone".to_string(),
        })
        .await
        .unwrap_err(),
    );
    assert_control_config_error(sdk.list_devices().await.unwrap_err());
    assert_control_config_error(
        sdk.list_device_services(&DeviceId::new("pc_001"))
            .await
            .unwrap_err(),
    );
    assert_control_config_error(
        sdk.create_session(CreateSessionInput {
            client_id: ClientId::new("phone_001"),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web"),
        })
        .await
        .unwrap_err(),
    );
}

#[tokio::test]
async fn facade_logout_clears_shared_token_store_without_control_request() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("https://127.0.0.1:4242")
        .build()
        .unwrap();
    sdk.token_store()
        .save_token(StoredToken {
            user_id: UserId::new("user_001"),
            access_token: "token.saved".to_string(),
            expire_at: 100,
        })
        .await
        .unwrap();

    sdk.logout().await.unwrap();

    assert_eq!(sdk.current_token().await.unwrap(), None);
}

#[tokio::test]
async fn facade_open_mobile_service_uses_shared_token_and_ephemeral_port() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:4242")
        .build()
        .unwrap();
    sdk.token_store()
        .save_token(StoredToken {
            user_id: UserId::new("user_001"),
            access_token: "token.mobile".to_string(),
            expire_at: 100,
        })
        .await
        .unwrap();

    let opened = sdk
        .open_mobile_service_in_memory(OpenMobileServiceInput {
            client_id: ClientId::new("phone_001"),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web"),
            local_port: 0,
        })
        .await
        .unwrap();

    assert_ne!(opened.forward().local_port(), 0);
}

#[tokio::test]
async fn facade_open_mobile_service_with_control_uses_shared_token_and_ephemeral_port() {
    let sdk = sdk_with_mobile_token().await;

    let opened = sdk
        .open_mobile_service_with_control(
            open_mobile_service_input(0),
            CertificateDer::from(vec![1, 2, 3]),
        )
        .await
        .unwrap();

    assert_ne!(opened.forward().local_port(), 0);
}

#[tokio::test]
async fn facade_open_mobile_service_p2p_or_relay_uses_shared_token_and_ephemeral_port() {
    let sdk = sdk_with_mobile_token().await;

    let opened = sdk
        .open_mobile_service_p2p_or_relay(open_mobile_service_input(0), p2p_or_relay_config())
        .await
        .unwrap();

    assert_ne!(opened.forward().local_port(), 0);
}

#[tokio::test]
async fn facade_open_mobile_service_requires_saved_token() {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:4242")
        .build()
        .unwrap();

    let err = sdk
        .open_mobile_service_in_memory(OpenMobileServiceInput {
            client_id: ClientId::new("phone_001"),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web"),
            local_port: 18080,
        })
        .await
        .unwrap_err();

    assert!(matches!(err, SdkError::NotAuthenticated));
}

#[test]
fn sdk_error_classifies_auth_related_statuses() {
    let unauthorized = SdkError::Control(ControlClientError::HttpStatus {
        status_code: StatusCode::from_u16(401),
        body: b"expired".to_vec(),
    });
    let forbidden = SdkError::Control(ControlClientError::HttpStatus {
        status_code: StatusCode::from_u16(403),
        body: b"forbidden".to_vec(),
    });

    assert_eq!(unauthorized.control_status_code(), Some(401));
    assert!(unauthorized.is_unauthorized());
    assert!(unauthorized.requires_reauthentication());
    assert!(!unauthorized.is_forbidden());

    assert_eq!(forbidden.control_status_code(), Some(403));
    assert!(forbidden.is_forbidden());
    assert!(!forbidden.requires_reauthentication());
    assert!(!forbidden.is_unauthorized());

    assert!(SdkError::NotAuthenticated.is_unauthorized());
    assert!(SdkError::NotAuthenticated.requires_reauthentication());
}

#[test]
fn builder_rejects_missing_or_empty_control_url() {
    let missing = MobileCodeConnectSdk::builder().build().unwrap_err();
    let empty = MobileCodeConnectSdk::builder()
        .control_url("   ")
        .build()
        .unwrap_err();

    assert!(matches!(missing, SdkError::InvalidConfig { .. }));
    assert!(matches!(empty, SdkError::InvalidConfig { .. }));
}

fn unique_temp_dir() -> std::path::PathBuf {
    static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let id = NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("mobilecode-connect-sdk-facade-{suffix}-{id}"))
}

fn assert_control_config_error(err: SdkError) {
    assert!(matches!(err, SdkError::Control(_)));
}

fn login_input() -> LoginInput {
    LoginInput {
        email: "facade@example.com".to_string(),
        password: "password-123".to_string(),
    }
}

fn register_input() -> RegisterInput {
    RegisterInput {
        email: "facade@example.com".to_string(),
        password: "password-123".to_string(),
        display_name: "Facade User".to_string(),
    }
}

async fn sdk_with_mobile_token() -> MobileCodeConnectSdk {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:4242")
        .build()
        .unwrap();
    sdk.token_store()
        .save_token(StoredToken {
            user_id: UserId::new("user_001"),
            access_token: "token.mobile".to_string(),
            expire_at: 100,
        })
        .await
        .unwrap();
    sdk
}

fn open_mobile_service_input(local_port: u16) -> OpenMobileServiceInput {
    OpenMobileServiceInput {
        client_id: ClientId::new("phone_001"),
        device_id: DeviceId::new("pc_001"),
        service_id: ServiceId::new("svc_web"),
        local_port,
    }
}

fn p2p_or_relay_config() -> P2pOrRelayTunnelConfig {
    P2pOrRelayTunnelConfig {
        relay_server_cert: CertificateDer::from(vec![1, 2, 3]),
        bind_addr: SocketAddr::from(([0, 0, 0, 0], 0)),
        candidate_timeout: Duration::from_millis(1500),
        probe_timeout: Duration::from_millis(1500),
        interval: Duration::from_millis(25),
        relay_fallback_delay: Duration::from_millis(300),
    }
}

fn server_login_input() -> ServerLoginInput {
    ServerLoginInput {
        device_id: DeviceId::new("pc_001"),
        device_name: "Office PC".to_string(),
        server_public_key: "server-public-key".to_string(),
    }
}

fn server_credential() -> StoredServerCredential {
    server_credential_for("http://127.0.0.1:4242")
}

fn server_credential_for(control_server: &str) -> StoredServerCredential {
    StoredServerCredential {
        control_server: control_server.to_string(),
        credential_id: "srv_cred_001".to_string(),
        device_id: DeviceId::new("pc_001"),
        device_name: "Office PC".to_string(),
        server_token: "server-token".to_string(),
        token_type: "Bearer".to_string(),
    }
}
