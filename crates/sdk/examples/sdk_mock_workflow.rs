use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use mobilecode_connect_control_client::{
    AgentSessionAssignment, AgentSessionStatus, AuthResponse, BrowserServerAuthExchangeRequest,
    BrowserServerAuthStartResponse, ControlClientError, ControllerDevice, CreateSessionRequest,
    CreateSessionResponse, DeviceServerAuthPollResponse, DeviceServerAuthStartResponse,
    LoginRequest, PollServerAuthRequest, RegisterControllerDeviceRequest, RegisterUserRequest,
    ServerAuthStatus, ServerCredentialResponse, StartServerAuthRequest, UpdatePasswordRequest,
};
use mobilecode_connect_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, Service, ServiceId, ServiceProtocol, SessionId,
    UserId,
};
use mobilecode_connect_sdk::{
    auth::{AuthSdk, LoginInput},
    client::ControlApi,
    controller::{ControllerApi, ControllerSdk, CreateSessionInput, RegisterControllerInput},
    server::{ServerApi, ServerRegistrationInput, ServerSdk},
    server_auth::{ServerAuthApi, ServerAuthSdk, ServerLoginInput},
    MobileCodeConnectSdk,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdk = MobileCodeConnectSdk::builder()
        .control_url("http://127.0.0.1:8080")
        .memory_token_store()
        .memory_server_credential_store()
        .build()?;

    let auth = AuthSdk::new(FakeControlApi::default(), sdk.token_store());
    let token = auth
        .login(LoginInput {
            email: "owner@example.com".to_string(),
            password: "password-123".to_string(),
        })
        .await?;
    println!("logged in user={} token_saved=true", token.user_id);

    let admin_token_shared = sdk.admin()?.current_token().await?.is_some();
    println!("admin facade sees saved user token={admin_token_shared}");

    let controller = ControllerSdk::new(FakeControllerApi::default(), sdk.token_store());
    let controller_device = controller
        .register_controller(RegisterControllerInput {
            client_id: ClientId::new("phone_001"),
            name: "Phone".to_string(),
        })
        .await?;
    println!("registered controller={}", controller_device.client_id);

    let server_auth = ServerAuthSdk::new(
        sdk.control_url(),
        FakeServerAuthApi::default(),
        sdk.server_credential_store(),
    );
    let pending = server_auth
        .start_browser_login(server_login_input())
        .await?;
    println!("server auth url={}", pending.auth_url);
    let credential = server_auth
        .complete_browser_login(pending, "mock-server-auth-code")
        .await?;
    println!(
        "server credential saved for device={}",
        credential.device_id
    );

    let server = ServerSdk::new(FakeServerApi::default(), sdk.server_credential_store());
    server
        .register_server(ServerRegistrationInput {
            device: controlled_device(),
            services: vec![web_service()],
            p2p_certificate_der: Some(vec![1, 2, 3]),
        })
        .await?;
    println!("registered server device=pc_001 services=1");

    let devices = controller.list_devices().await?;
    let services = controller
        .list_device_services(&DeviceId::new("pc_001"))
        .await?;
    let session = controller
        .create_session(CreateSessionInput {
            client_id: controller_device.client_id,
            device_id: devices[0].device_id.clone(),
            service_id: services[0].service_id.clone(),
        })
        .await?;
    println!(
        "created session={} relay={}",
        session.session_id, session.relay_addr
    );

    Ok(())
}

#[derive(Clone, Default)]
struct FakeControlApi {
    bearer_tokens: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl ControlApi for FakeControlApi {
    fn set_bearer_token(&mut self, bearer_token: String) {
        self.bearer_tokens
            .lock()
            .expect("fake control state poisoned")
            .push(bearer_token);
    }

    async fn register_user(
        &mut self,
        _request: RegisterUserRequest,
    ) -> Result<AuthResponse, ControlClientError> {
        Ok(auth_response())
    }

    async fn login(&mut self, _request: LoginRequest) -> Result<AuthResponse, ControlClientError> {
        Ok(auth_response())
    }

    async fn update_password(
        &mut self,
        _request: UpdatePasswordRequest,
    ) -> Result<(), ControlClientError> {
        Ok(())
    }
}

#[derive(Clone, Default)]
struct FakeControllerApi;

#[async_trait]
impl ControllerApi for FakeControllerApi {
    fn set_bearer_token(&mut self, _bearer_token: String) {}

    async fn register_controller(
        &mut self,
        request: RegisterControllerDeviceRequest,
    ) -> Result<ControllerDevice, ControlClientError> {
        Ok(ControllerDevice {
            user_id: UserId::new("user_001"),
            client_id: ClientId::new(request.client_id),
            name: request.name,
        })
    }

    async fn list_devices(&mut self) -> Result<Vec<Device>, ControlClientError> {
        Ok(vec![controlled_device()])
    }

    async fn list_device_services(
        &mut self,
        _device_id: &DeviceId,
    ) -> Result<Vec<Service>, ControlClientError> {
        Ok(vec![web_service()])
    }

    async fn create_session(
        &mut self,
        _request: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, ControlClientError> {
        Ok(CreateSessionResponse {
            session_id: SessionId::new("sess_001"),
            access_token: "access-token".to_string(),
            relay_token: "relay-token".to_string(),
            relay_addr: "127.0.0.1:4443".to_string(),
            punch_addr: "127.0.0.1:3478".to_string(),
            agent_p2p_cert_der: Some(vec![1, 2, 3]),
            expire_at: 1_900_000_000,
        })
    }
}

#[derive(Clone, Default)]
struct FakeServerAuthApi;

#[async_trait]
impl ServerAuthApi for FakeServerAuthApi {
    async fn start_browser_server_auth(
        &mut self,
        _request: StartServerAuthRequest,
    ) -> Result<BrowserServerAuthStartResponse, ControlClientError> {
        Ok(BrowserServerAuthStartResponse {
            session_id: "srv_auth_001".to_string(),
            auth_url: "http://127.0.0.1:8080/server-auth/browser/approve?session_id=srv_auth_001"
                .to_string(),
            expires_in: 600,
        })
    }

    async fn exchange_browser_server_auth(
        &mut self,
        _request: BrowserServerAuthExchangeRequest,
    ) -> Result<ServerCredentialResponse, ControlClientError> {
        Ok(server_credential_response())
    }

    async fn start_device_server_auth(
        &mut self,
        _request: StartServerAuthRequest,
    ) -> Result<DeviceServerAuthStartResponse, ControlClientError> {
        Ok(DeviceServerAuthStartResponse {
            device_code: "device-code-001".to_string(),
            user_code: "ABCD-EFGH".to_string(),
            verification_uri: "http://127.0.0.1:8080/server-auth/device".to_string(),
            verification_uri_complete:
                "http://127.0.0.1:8080/server-auth/device?user_code=ABCD-EFGH".to_string(),
            expires_in: 600,
            interval: 0,
        })
    }

    async fn poll_device_server_auth(
        &mut self,
        _request: PollServerAuthRequest,
    ) -> Result<DeviceServerAuthPollResponse, ControlClientError> {
        Ok(DeviceServerAuthPollResponse {
            status: ServerAuthStatus::Approved,
            interval: 0,
            credential: Some(server_credential_response()),
        })
    }
}

#[derive(Clone, Default)]
struct FakeServerApi;

#[async_trait]
impl ServerApi for FakeServerApi {
    fn set_bearer_token(&mut self, _bearer_token: String) {}

    async fn register_device(&mut self, _device: Device) -> Result<(), ControlClientError> {
        Ok(())
    }

    async fn register_services(
        &mut self,
        _services: Vec<Service>,
    ) -> Result<(), ControlClientError> {
        Ok(())
    }

    async fn register_p2p_certificate(
        &mut self,
        _device_id: &DeviceId,
        _certificate_der: Vec<u8>,
    ) -> Result<(), ControlClientError> {
        Ok(())
    }

    async fn list_agent_sessions(
        &mut self,
        _device_id: &DeviceId,
    ) -> Result<Vec<AgentSessionAssignment>, ControlClientError> {
        Ok(vec![assignment(AgentSessionStatus::Pending)])
    }

    async fn claim_agent_session(
        &mut self,
        _session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        Ok(assignment(AgentSessionStatus::Claimed))
    }

    async fn mark_agent_session_bound(
        &mut self,
        _session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        Ok(assignment(AgentSessionStatus::Bound))
    }

    async fn close_session(
        &mut self,
        _session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        Ok(assignment(AgentSessionStatus::Closed))
    }
}

fn auth_response() -> AuthResponse {
    AuthResponse {
        user_id: UserId::new("user_001"),
        access_token: "mock-user-token".to_string(),
        expire_at: 1_900_000_000,
    }
}

fn server_login_input() -> ServerLoginInput {
    ServerLoginInput {
        device_id: DeviceId::new("pc_001"),
        device_name: "Office PC".to_string(),
        server_public_key: "mock-server-public-key".to_string(),
    }
}

fn server_credential_response() -> ServerCredentialResponse {
    ServerCredentialResponse {
        credential_id: "srv_cred_001".to_string(),
        device_id: DeviceId::new("pc_001"),
        server_token: "mock-server-token".to_string(),
        token_type: "Bearer".to_string(),
    }
}

fn controlled_device() -> Device {
    Device {
        device_id: DeviceId::new("pc_001"),
        user_id: UserId::new("user_001"),
        name: "Office PC".to_string(),
        status: DeviceStatus::Online,
        agent_version: "0.1.0".to_string(),
    }
}

fn web_service() -> Service {
    Service {
        service_id: ServiceId::new("svc_web"),
        device_id: DeviceId::new("pc_001"),
        name: "Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: 3000,
    }
}

fn assignment(status: AgentSessionStatus) -> AgentSessionAssignment {
    AgentSessionAssignment {
        session_id: SessionId::new("sess_001"),
        user_id: UserId::new("user_001"),
        device_id: DeviceId::new("pc_001"),
        service_id: ServiceId::new("svc_web"),
        client_id: ClientId::new("phone_001"),
        relay_token: "relay-token".to_string(),
        relay_addr: "127.0.0.1:4443".to_string(),
        punch_addr: "127.0.0.1:3478".to_string(),
        expire_at: 1_900_000_000,
        status,
        grant_id: None,
        grant_revocation_version: None,
        grant_service_id: None,
    }
}
