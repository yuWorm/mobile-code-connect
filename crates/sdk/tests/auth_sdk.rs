use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use quic_tunnel_control_client::{
    AuthResponse, ControlClientError, LoginRequest, RegisterUserRequest, UpdatePasswordRequest,
};
use quic_tunnel_protocol::UserId;
use quic_tunnel_sdk::{
    auth::{AuthSdk, LoginInput, RegisterInput},
    client::ControlApi,
    store::{MemoryTokenStore, StoredToken, TokenStore},
};

#[derive(Clone)]
struct FakeControlApi {
    state: Arc<Mutex<FakeControlState>>,
    response: AuthResponse,
}

#[derive(Debug, Default)]
struct FakeControlState {
    register_requests: Vec<RegisterUserRequest>,
    login_requests: Vec<LoginRequest>,
    password_requests: Vec<UpdatePasswordRequest>,
    bearer_tokens: Vec<String>,
}

impl FakeControlApi {
    fn new(response: AuthResponse) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeControlState::default())),
            response,
        }
    }
}

#[async_trait]
impl ControlApi for FakeControlApi {
    fn set_bearer_token(&mut self, bearer_token: String) {
        self.state
            .lock()
            .expect("fake control state poisoned")
            .bearer_tokens
            .push(bearer_token);
    }

    async fn register_user(
        &mut self,
        request: RegisterUserRequest,
    ) -> Result<AuthResponse, ControlClientError> {
        self.state
            .lock()
            .expect("fake control state poisoned")
            .register_requests
            .push(request);
        Ok(self.response.clone())
    }

    async fn login(&mut self, request: LoginRequest) -> Result<AuthResponse, ControlClientError> {
        self.state
            .lock()
            .expect("fake control state poisoned")
            .login_requests
            .push(request);
        Ok(self.response.clone())
    }

    async fn update_password(
        &mut self,
        request: UpdatePasswordRequest,
    ) -> Result<(), ControlClientError> {
        self.state
            .lock()
            .expect("fake control state poisoned")
            .password_requests
            .push(request);
        Ok(())
    }
}

#[tokio::test]
async fn register_persists_token_and_authorizes_follow_up_calls() {
    let fake = FakeControlApi::new(auth_response("user_001", "token.registered", 100));
    let store = MemoryTokenStore::default();
    let sdk = AuthSdk::new(fake.clone(), store.clone());

    let token = sdk
        .register(RegisterInput {
            email: "alice@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Alice".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(
        token,
        StoredToken {
            user_id: UserId::new("user_001"),
            access_token: "token.registered".to_string(),
            expire_at: 100,
        }
    );
    assert_eq!(store.load_token().await.unwrap(), Some(token));

    let state = fake.state.lock().expect("fake control state poisoned");
    assert_eq!(
        state.register_requests,
        vec![RegisterUserRequest {
            email: "alice@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Alice".to_string(),
        }]
    );
    assert_eq!(state.bearer_tokens, vec!["token.registered".to_string()]);
}

#[tokio::test]
async fn login_persists_token_and_updates_control_authorization() {
    let fake = FakeControlApi::new(auth_response("user_002", "token.logged-in", 200));
    let store = MemoryTokenStore::default();
    let sdk = AuthSdk::new(fake.clone(), store.clone());

    let token = sdk
        .login(LoginInput {
            email: "bob@example.com".to_string(),
            password: "password-456".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(
        store.load_token().await.unwrap(),
        Some(StoredToken {
            user_id: UserId::new("user_002"),
            access_token: "token.logged-in".to_string(),
            expire_at: 200,
        })
    );
    assert_eq!(token.access_token, "token.logged-in");

    let state = fake.state.lock().expect("fake control state poisoned");
    assert_eq!(
        state.login_requests,
        vec![LoginRequest {
            email: "bob@example.com".to_string(),
            password: "password-456".to_string(),
        }]
    );
    assert_eq!(state.bearer_tokens, vec!["token.logged-in".to_string()]);
}

#[tokio::test]
async fn update_password_uses_the_saved_token() {
    let fake = FakeControlApi::new(auth_response("user_003", "unused", 300));
    let store = MemoryTokenStore::default();
    store
        .save_token(StoredToken {
            user_id: UserId::new("user_003"),
            access_token: "token.saved".to_string(),
            expire_at: 300,
        })
        .await
        .unwrap();
    let sdk = AuthSdk::new(fake.clone(), store);

    sdk.update_password(Some("old-password".to_string()), "new-password")
        .await
        .unwrap();

    let state = fake.state.lock().expect("fake control state poisoned");
    assert_eq!(state.bearer_tokens, vec!["token.saved".to_string()]);
    assert_eq!(
        state.password_requests,
        vec![UpdatePasswordRequest {
            current_password: Some("old-password".to_string()),
            new_password: "new-password".to_string(),
        }]
    );
}

#[tokio::test]
async fn logout_clears_the_saved_token() {
    let fake = FakeControlApi::new(auth_response("user_004", "unused", 400));
    let store = MemoryTokenStore::default();
    store
        .save_token(StoredToken {
            user_id: UserId::new("user_004"),
            access_token: "token.saved".to_string(),
            expire_at: 400,
        })
        .await
        .unwrap();
    let sdk = AuthSdk::new(fake, store.clone());

    sdk.logout().await.unwrap();

    assert_eq!(store.load_token().await.unwrap(), None);
}

fn auth_response(user_id: &str, access_token: &str, expire_at: u64) -> AuthResponse {
    AuthResponse {
        user_id: UserId::new(user_id),
        access_token: access_token.to_string(),
        expire_at,
    }
}
