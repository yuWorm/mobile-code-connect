use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use mobilecode_connect_control_client::{
    BrowserServerAuthExchangeRequest, BrowserServerAuthStartResponse, ControlClientError,
    DeviceServerAuthPollResponse, DeviceServerAuthStartResponse, PollServerAuthRequest,
    ServerAuthStatus, ServerCredentialResponse, StartServerAuthRequest,
};
use mobilecode_connect_protocol::DeviceId;
use mobilecode_connect_sdk::server_auth::{
    FileServerCredentialStore, MemoryServerCredentialStore, ServerAuthApi, ServerAuthSdk,
    ServerCredentialStore, ServerLoginInput, StoredServerCredential,
};

#[derive(Clone)]
struct FakeServerAuthApi {
    state: Arc<Mutex<FakeServerAuthState>>,
}

#[derive(Debug)]
struct FakeServerAuthState {
    browser_start_requests: Vec<StartServerAuthRequest>,
    browser_exchange_requests: Vec<BrowserServerAuthExchangeRequest>,
    device_start_requests: Vec<StartServerAuthRequest>,
    poll_requests: Vec<PollServerAuthRequest>,
    device_poll_responses: Vec<DeviceServerAuthPollResponse>,
}

impl FakeServerAuthApi {
    fn new(device_poll_responses: Vec<DeviceServerAuthPollResponse>) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeServerAuthState {
                browser_start_requests: Vec::new(),
                browser_exchange_requests: Vec::new(),
                device_start_requests: Vec::new(),
                poll_requests: Vec::new(),
                device_poll_responses,
            })),
        }
    }
}

#[async_trait]
impl ServerAuthApi for FakeServerAuthApi {
    async fn start_browser_server_auth(
        &mut self,
        request: StartServerAuthRequest,
    ) -> Result<BrowserServerAuthStartResponse, ControlClientError> {
        self.state
            .lock()
            .expect("fake server auth state poisoned")
            .browser_start_requests
            .push(request);
        Ok(BrowserServerAuthStartResponse {
            session_id: "srv_auth_browser".to_string(),
            auth_url: "http://control/server-auth/browser/approve?session_id=srv_auth_browser"
                .to_string(),
            expires_in: 600,
        })
    }

    async fn exchange_browser_server_auth(
        &mut self,
        request: BrowserServerAuthExchangeRequest,
    ) -> Result<ServerCredentialResponse, ControlClientError> {
        self.state
            .lock()
            .expect("fake server auth state poisoned")
            .browser_exchange_requests
            .push(request);
        Ok(server_credential(
            "srv_cred_browser",
            "server-token-browser",
        ))
    }

    async fn start_device_server_auth(
        &mut self,
        request: StartServerAuthRequest,
    ) -> Result<DeviceServerAuthStartResponse, ControlClientError> {
        self.state
            .lock()
            .expect("fake server auth state poisoned")
            .device_start_requests
            .push(request);
        Ok(DeviceServerAuthStartResponse {
            device_code: "device-code-001".to_string(),
            user_code: "ABCD-EFGH".to_string(),
            verification_uri: "http://control/server-auth/device".to_string(),
            verification_uri_complete: "http://control/server-auth/device?user_code=ABCD-EFGH"
                .to_string(),
            expires_in: 600,
            interval: 0,
        })
    }

    async fn poll_device_server_auth(
        &mut self,
        request: PollServerAuthRequest,
    ) -> Result<DeviceServerAuthPollResponse, ControlClientError> {
        let mut state = self.state.lock().expect("fake server auth state poisoned");
        state.poll_requests.push(request);
        Ok(state.device_poll_responses.remove(0))
    }
}

#[tokio::test]
async fn browser_login_persists_the_exchanged_server_credential() {
    let api = FakeServerAuthApi::new(Vec::new());
    let store = MemoryServerCredentialStore::default();
    let sdk = ServerAuthSdk::new("http://control.local", api.clone(), store.clone());

    let pending = sdk.start_browser_login(login_input()).await.unwrap();
    assert_eq!(
        pending.auth_url,
        "http://control/server-auth/browser/approve?session_id=srv_auth_browser"
    );

    let credential = sdk
        .complete_browser_login(pending, "auth-code-123")
        .await
        .unwrap();

    assert_eq!(
        credential,
        stored_credential("srv_cred_browser", "server-token-browser")
    );
    assert_eq!(store.load_credential().await.unwrap(), Some(credential));

    let state = api.state.lock().expect("fake server auth state poisoned");
    assert_eq!(state.browser_start_requests, vec![start_request()]);
    assert_eq!(
        state.browser_exchange_requests,
        vec![BrowserServerAuthExchangeRequest {
            session_id: "srv_auth_browser".to_string(),
            server_auth_code: "auth-code-123".to_string(),
            server_public_key: "server-public-key".to_string(),
        }]
    );
}

#[tokio::test]
async fn device_code_login_polls_until_approved_and_persists_the_credential() {
    let api = FakeServerAuthApi::new(vec![
        DeviceServerAuthPollResponse {
            status: ServerAuthStatus::AuthorizationPending,
            interval: 0,
            credential: None,
        },
        DeviceServerAuthPollResponse {
            status: ServerAuthStatus::Approved,
            interval: 0,
            credential: Some(server_credential("srv_cred_device", "server-token-device")),
        },
    ]);
    let store = MemoryServerCredentialStore::default();
    let sdk = ServerAuthSdk::new("http://control.local", api.clone(), store.clone());

    let pending = sdk.start_device_code_login(login_input()).await.unwrap();
    assert_eq!(pending.user_code, "ABCD-EFGH");

    let credential = sdk
        .complete_device_code_login(pending, Duration::from_millis(0))
        .await
        .unwrap();

    assert_eq!(
        credential,
        stored_credential("srv_cred_device", "server-token-device")
    );
    assert_eq!(store.load_credential().await.unwrap(), Some(credential));

    let state = api.state.lock().expect("fake server auth state poisoned");
    assert_eq!(state.device_start_requests, vec![start_request()]);
    assert_eq!(
        state.poll_requests,
        vec![
            PollServerAuthRequest {
                device_code: "device-code-001".to_string(),
                server_public_key: "server-public-key".to_string(),
            },
            PollServerAuthRequest {
                device_code: "device-code-001".to_string(),
                server_public_key: "server-public-key".to_string(),
            },
        ]
    );
}

#[tokio::test]
async fn file_server_credential_store_roundtrips_with_private_permissions() {
    let dir = unique_temp_dir();
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let path = dir.join("agentd-credential.json");
    let store = FileServerCredentialStore::new(path.clone());
    let credential = stored_credential("srv_cred_file", "server-token-file");

    store.save_credential(credential.clone()).await.unwrap();
    assert_eq!(store.load_credential().await.unwrap(), Some(credential));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = tokio::fs::metadata(&path)
            .await
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    store.clear_credential().await.unwrap();
    assert_eq!(store.load_credential().await.unwrap(), None);

    tokio::fs::remove_dir_all(&dir).await.unwrap();
}

fn login_input() -> ServerLoginInput {
    ServerLoginInput {
        device_id: DeviceId::new("pc_001"),
        device_name: "Office PC".to_string(),
        server_public_key: "server-public-key".to_string(),
    }
}

fn start_request() -> StartServerAuthRequest {
    StartServerAuthRequest {
        device_id: DeviceId::new("pc_001"),
        device_name: "Office PC".to_string(),
        server_public_key: "server-public-key".to_string(),
    }
}

fn server_credential(credential_id: &str, server_token: &str) -> ServerCredentialResponse {
    ServerCredentialResponse {
        credential_id: credential_id.to_string(),
        device_id: DeviceId::new("pc_001"),
        server_token: server_token.to_string(),
        token_type: "bearer".to_string(),
    }
}

fn stored_credential(credential_id: &str, server_token: &str) -> StoredServerCredential {
    StoredServerCredential {
        control_server: "http://control.local".to_string(),
        credential_id: credential_id.to_string(),
        device_id: DeviceId::new("pc_001"),
        device_name: "Office PC".to_string(),
        server_token: server_token.to_string(),
        token_type: "bearer".to_string(),
    }
}

fn unique_temp_dir() -> std::path::PathBuf {
    static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let id = NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("mobilecode-connect-sdk-server-auth-{suffix}-{id}"))
}
