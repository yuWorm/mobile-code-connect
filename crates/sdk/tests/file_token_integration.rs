use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use quic_tunnel_control_client::{
    AuthResponse, ControlClientError, LoginRequest, RegisterUserRequest, UpdatePasswordRequest,
};
use quic_tunnel_protocol::{ClientId, UserId};
use quic_tunnel_sdk::{
    auth::{AuthSdk, LoginInput},
    client::ControlApi,
    mobile::{MobileTunnelConfig, MobileTunnelSdk},
    store::{FileTokenStore, TokenStore},
    HttpControlClientOptions,
};

#[derive(Clone)]
struct FakeAuthApi {
    bearer_tokens: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl ControlApi for FakeAuthApi {
    fn set_bearer_token(&mut self, bearer_token: String) {
        self.bearer_tokens
            .lock()
            .expect("fake auth state poisoned")
            .push(bearer_token);
    }

    async fn register_user(
        &mut self,
        _request: RegisterUserRequest,
    ) -> Result<AuthResponse, ControlClientError> {
        unreachable!("test only exercises login")
    }

    async fn login(&mut self, _request: LoginRequest) -> Result<AuthResponse, ControlClientError> {
        Ok(AuthResponse {
            user_id: UserId::new("user_001"),
            access_token: "token.file".to_string(),
            expire_at: 100,
        })
    }

    async fn update_password(
        &mut self,
        _request: UpdatePasswordRequest,
    ) -> Result<(), ControlClientError> {
        unreachable!("test only exercises login")
    }
}

#[tokio::test]
async fn auth_sdk_login_file_token_can_bootstrap_mobile_tunnel() {
    let dir = unique_temp_dir();
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let path = dir.join("token.json");
    let store = FileTokenStore::new(path.clone());
    let bearer_tokens = Arc::new(Mutex::new(Vec::new()));
    let auth = AuthSdk::new(
        FakeAuthApi {
            bearer_tokens: bearer_tokens.clone(),
        },
        store.clone(),
    );

    auth.login(LoginInput {
        email: "member@example.com".to_string(),
        password: "password-123".to_string(),
    })
    .await
    .unwrap();

    assert_eq!(
        bearer_tokens
            .lock()
            .expect("fake auth state poisoned")
            .as_slice(),
        ["token.file"]
    );
    assert_eq!(
        FileTokenStore::new(path)
            .load_token()
            .await
            .unwrap()
            .unwrap()
            .access_token,
        "token.file"
    );

    let tunnel = MobileTunnelSdk::start_in_memory(
        MobileTunnelConfig {
            control_server_url: "http://127.0.0.1:4242".to_string(),
            client_id: ClientId::new("phone_001"),
            control_client_options: HttpControlClientOptions::default(),
        },
        store,
    )
    .await
    .unwrap();
    assert_eq!(tunnel.status().active_forwards, 0);

    tokio::fs::remove_dir_all(&dir).await.unwrap();
}

fn unique_temp_dir() -> std::path::PathBuf {
    static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let id = NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "quic-test-sdk-file-token-integration-{suffix}-{id}"
    ))
}
