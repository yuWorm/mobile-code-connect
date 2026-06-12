use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Method, Request},
    Router,
};
use mobilecode_connect_control::{routes::routes, state::ControlState};
use mobilecode_connect_control_client::{
    AgentSessionAssignment, AgentSessionStatus, AuthResponse, BrowserServerAuthApprovalResponse,
    BrowserServerAuthExchangeRequest, BrowserServerAuthStartResponse, ControlClientError,
    ControllerDevice, CreateSessionRequest, CreateSessionResponse, DeviceServerAuthPollResponse,
    DeviceServerAuthStartResponse, LoginRequest, PollServerAuthRequest,
    RegisterControllerDeviceRequest, RegisterP2pCertificateRequest, RegisterUserRequest,
    ServerCredentialResponse, StartServerAuthRequest, UpdatePasswordRequest,
};
use mobilecode_connect_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, Service, ServiceId, ServiceProtocol, SessionId,
    UserId,
};
use mobilecode_connect_sdk::{
    auth::AuthSdk,
    client::ControlApi,
    controller::ControllerApi,
    server::ServerApi,
    server_auth::{ServerAuthApi, ServerAuthSdk},
    store::TokenStore,
    ControllerSdk, CreateSessionInput, LoginInput, MobileCodeConnectSdk, RegisterControllerInput,
    RegisterInput, ServerLoginInput, ServerRegistrationInput, ServerSdk,
};
use serde::{de::DeserializeOwned, Serialize};
use tower::ServiceExt;

#[tokio::test]
async fn sdk_facade_runs_full_live_workflow_against_in_process_control_routes() {
    let app = routes(ControlState::new(
        "dev-secret",
        "127.0.0.1:4443",
        "127.0.0.1:3478",
    ));
    let dir = unique_temp_dir();
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:1")
        .token_file(dir.join("user-token.json"))
        .server_credential_file(dir.join("server-credential.json"))
        .build()
        .unwrap();

    let auth = AuthSdk::new(InProcessControlClient::new(app.clone()), sdk.token_store());
    let token = auth
        .register(RegisterInput {
            email: "sdk-live-owner@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "SDK Live Owner".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        auth.login(LoginInput {
            email: "sdk-live-owner@example.com".to_string(),
            password: "password-123".to_string(),
        })
        .await
        .unwrap()
        .user_id,
        token.user_id
    );
    assert_saved_token(&sdk, &token.user_id).await;
    assert!(sdk
        .admin()
        .unwrap()
        .current_token()
        .await
        .unwrap()
        .is_some());

    let controller =
        ControllerSdk::new(InProcessControlClient::new(app.clone()), sdk.token_store());
    let controller_device = controller
        .register_controller(RegisterControllerInput {
            client_id: ClientId::new("phone_001"),
            name: "Phone".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(controller_device.client_id, ClientId::new("phone_001"));

    let server_auth = ServerAuthSdk::new(
        sdk.control_url(),
        InProcessControlClient::new(app.clone()),
        sdk.server_credential_store(),
    );
    let pending = server_auth
        .start_browser_login(ServerLoginInput {
            device_id: DeviceId::new("pc_001"),
            device_name: "Office PC".to_string(),
            server_public_key: "sdk-live-server-public-key".to_string(),
        })
        .await
        .unwrap();
    let approval =
        approve_browser_server_auth(app.clone(), &pending.session_id, &token.access_token).await;
    let credential = server_auth
        .complete_browser_login(pending, approval.server_auth_code)
        .await
        .unwrap();
    assert_eq!(credential.device_id, DeviceId::new("pc_001"));
    assert!(credential.credential_id.starts_with("srv_cred_"));
    assert_eq!(
        sdk.current_server_credential().await.unwrap(),
        Some(credential.clone())
    );

    let service = Service {
        service_id: ServiceId::new("svc_web_3000"),
        device_id: DeviceId::new("pc_001"),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: 3000,
    };
    let server = ServerSdk::new(
        InProcessControlClient::new(app.clone()),
        sdk.server_credential_store(),
    );
    server
        .register_server(ServerRegistrationInput {
            device: Device {
                device_id: DeviceId::new("pc_001"),
                user_id: UserId::new("ignored-by-agent-token"),
                name: "Office PC".to_string(),
                status: DeviceStatus::Online,
                agent_version: "sdk-live-workflow".to_string(),
            },
            services: vec![service.clone()],
            p2p_certificate_der: Some(vec![1, 2, 3, 4]),
        })
        .await
        .unwrap();

    assert_eq!(
        controller.list_devices().await.unwrap()[0].device_id,
        service.device_id
    );
    assert_eq!(
        controller
            .list_device_services(&DeviceId::new("pc_001"))
            .await
            .unwrap(),
        vec![service.clone()]
    );

    let session = controller
        .create_session(CreateSessionInput {
            client_id: ClientId::new("phone_001"),
            device_id: DeviceId::new("pc_001"),
            service_id: service.service_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(session.relay_addr, "127.0.0.1:4443");
    assert_eq!(session.punch_addr, "127.0.0.1:3478");
    assert_eq!(session.agent_p2p_cert_der, Some(vec![1, 2, 3, 4]));

    let agent_sessions = server.list_sessions().await.unwrap();
    assert_eq!(agent_sessions.len(), 1);
    assert_eq!(agent_sessions[0].session_id, session.session_id);
    assert_eq!(agent_sessions[0].status, AgentSessionStatus::Pending);
    assert_eq!(
        server
            .claim_session(&session.session_id)
            .await
            .unwrap()
            .status,
        AgentSessionStatus::Claimed
    );
    assert_eq!(
        server
            .mark_session_bound(&session.session_id)
            .await
            .unwrap()
            .status,
        AgentSessionStatus::Bound
    );
    assert_eq!(
        server
            .close_session(&session.session_id)
            .await
            .unwrap()
            .status,
        AgentSessionStatus::Closed
    );

    tokio::fs::remove_dir_all(&dir).await.unwrap();
}

#[derive(Clone)]
struct InProcessControlClient {
    app: Router,
    bearer_token: Option<String>,
}

impl InProcessControlClient {
    fn new(app: Router) -> Self {
        Self {
            app,
            bearer_token: None,
        }
    }

    fn with_bearer_token(app: Router, bearer_token: impl Into<String>) -> Self {
        Self {
            app,
            bearer_token: Some(bearer_token.into()),
        }
    }

    async fn get_json<R>(&self, uri: impl Into<String>) -> Result<R, ControlClientError>
    where
        R: DeserializeOwned,
    {
        self.request_json(Method::GET, uri.into(), Option::<&()>::None)
            .await
    }

    async fn post_json<T, R>(
        &self,
        uri: impl Into<String>,
        payload: &T,
    ) -> Result<R, ControlClientError>
    where
        T: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.request_json(Method::POST, uri.into(), Some(payload))
            .await
    }

    async fn post_empty<T>(
        &self,
        uri: impl Into<String>,
        payload: &T,
    ) -> Result<(), ControlClientError>
    where
        T: Serialize + ?Sized,
    {
        let _ = self
            .request(Method::POST, uri.into(), Some(serde_json::to_vec(payload)?))
            .await?;
        Ok(())
    }

    async fn request_json<T, R>(
        &self,
        method: Method,
        uri: String,
        payload: Option<&T>,
    ) -> Result<R, ControlClientError>
    where
        T: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let body = match payload {
            Some(payload) => Some(serde_json::to_vec(payload)?),
            None => None,
        };
        let body = self.request(method, uri, body).await?;
        Ok(serde_json::from_slice(&body)?)
    }

    async fn request(
        &self,
        method: Method,
        uri: String,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, ControlClientError> {
        let mut builder = Request::builder().method(method).uri(uri.clone());
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        if let Some(token) = &self.bearer_token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        let request = builder
            .body(match body {
                Some(body) => Body::from(body),
                None => Body::empty(),
            })
            .map_err(|error| ControlClientError::MalformedResponse {
                reason: format!("build in-process request {uri}: {error}"),
            })?;
        let response = self.app.clone().oneshot(request).await.map_err(|error| {
            ControlClientError::MalformedResponse {
                reason: format!("route in-process request {uri}: {error}"),
            }
        })?;
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .map_err(|error| ControlClientError::MalformedResponse {
                reason: format!("read in-process response {uri}: {error}"),
            })?
            .to_vec();
        if !status.is_success() {
            return Err(ControlClientError::MalformedResponse {
                reason: format!("in-process {uri} returned {status}: {body:?}"),
            });
        }
        Ok(body)
    }
}

#[async_trait]
impl ControlApi for InProcessControlClient {
    fn set_bearer_token(&mut self, bearer_token: String) {
        self.bearer_token = Some(bearer_token);
    }

    async fn register_user(
        &mut self,
        request: RegisterUserRequest,
    ) -> Result<AuthResponse, ControlClientError> {
        self.post_json("/auth/register", &request).await
    }

    async fn login(&mut self, request: LoginRequest) -> Result<AuthResponse, ControlClientError> {
        self.post_json("/auth/login", &request).await
    }

    async fn update_password(
        &mut self,
        request: UpdatePasswordRequest,
    ) -> Result<(), ControlClientError> {
        self.post_empty("/auth/password", &request).await
    }
}

#[async_trait]
impl ControllerApi for InProcessControlClient {
    fn set_bearer_token(&mut self, bearer_token: String) {
        self.bearer_token = Some(bearer_token);
    }

    async fn register_controller(
        &mut self,
        request: RegisterControllerDeviceRequest,
    ) -> Result<ControllerDevice, ControlClientError> {
        self.post_json("/controllers/register", &request).await
    }

    async fn list_devices(&mut self) -> Result<Vec<Device>, ControlClientError> {
        self.get_json("/mobile/devices").await
    }

    async fn list_device_services(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<Vec<Service>, ControlClientError> {
        self.get_json(format!("/mobile/devices/{device_id}/services"))
            .await
    }

    async fn create_session(
        &mut self,
        request: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, ControlClientError> {
        self.post_json("/sessions", &request).await
    }
}

#[async_trait]
impl ServerAuthApi for InProcessControlClient {
    async fn start_browser_server_auth(
        &mut self,
        request: StartServerAuthRequest,
    ) -> Result<BrowserServerAuthStartResponse, ControlClientError> {
        self.post_json("/server-auth/browser/start", &request).await
    }

    async fn exchange_browser_server_auth(
        &mut self,
        request: BrowserServerAuthExchangeRequest,
    ) -> Result<ServerCredentialResponse, ControlClientError> {
        self.post_json("/server-auth/browser/exchange", &request)
            .await
    }

    async fn start_device_server_auth(
        &mut self,
        request: StartServerAuthRequest,
    ) -> Result<DeviceServerAuthStartResponse, ControlClientError> {
        self.post_json("/server-auth/device/start", &request).await
    }

    async fn poll_device_server_auth(
        &mut self,
        request: PollServerAuthRequest,
    ) -> Result<DeviceServerAuthPollResponse, ControlClientError> {
        self.post_json("/server-auth/device/poll", &request).await
    }
}

#[async_trait]
impl ServerApi for InProcessControlClient {
    fn set_bearer_token(&mut self, bearer_token: String) {
        self.bearer_token = Some(bearer_token);
    }

    async fn register_device(&mut self, device: Device) -> Result<(), ControlClientError> {
        self.post_empty("/agent/register", &device).await
    }

    async fn register_services(
        &mut self,
        services: Vec<Service>,
    ) -> Result<(), ControlClientError> {
        self.post_empty("/agent/services", &services).await
    }

    async fn register_p2p_certificate(
        &mut self,
        device_id: &DeviceId,
        certificate_der: Vec<u8>,
    ) -> Result<(), ControlClientError> {
        self.post_empty(
            format!("/agent/devices/{device_id}/p2p-cert"),
            &RegisterP2pCertificateRequest { certificate_der },
        )
        .await
    }

    async fn list_agent_sessions(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<Vec<AgentSessionAssignment>, ControlClientError> {
        self.get_json(format!("/agent/devices/{device_id}/sessions"))
            .await
    }

    async fn claim_agent_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        self.post_json(format!("/agent/sessions/{session_id}/claim"), &())
            .await
    }

    async fn mark_agent_session_bound(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        self.post_json(format!("/agent/sessions/{session_id}/bound"), &())
            .await
    }

    async fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        self.post_json(format!("/sessions/{session_id}/close"), &())
            .await
    }
}

async fn approve_browser_server_auth(
    app: Router,
    session_id: &str,
    bearer_token: &str,
) -> BrowserServerAuthApprovalResponse {
    InProcessControlClient::with_bearer_token(app, bearer_token)
        .get_json(format!(
            "/server-auth/browser/approve?session_id={session_id}"
        ))
        .await
        .unwrap()
}

async fn assert_saved_token(sdk: &MobileCodeConnectSdk, user_id: &UserId) {
    let stored = sdk.token_store().load_token().await.unwrap().unwrap();
    assert_eq!(&stored.user_id, user_id);
    assert!(!stored.access_token.is_empty());
    assert!(stored.expire_at > 0);
}

fn unique_temp_dir() -> std::path::PathBuf {
    static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let id = NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("quic-test-sdk-live-workflow-{suffix}-{id}"))
}
