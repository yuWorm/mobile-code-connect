use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use quic_tunnel_control_client::{AgentSessionAssignment, AgentSessionStatus, ControlClientError};
use quic_tunnel_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, Service, ServiceId, ServiceProtocol, SessionId,
    UserId,
};
use quic_tunnel_sdk::{
    server::{ServerApi, ServerRegistrationInput, ServerSdk},
    server_auth::{MemoryServerCredentialStore, ServerCredentialStore, StoredServerCredential},
    SdkError,
};

#[derive(Clone)]
struct FakeServerApi {
    state: Arc<Mutex<FakeServerState>>,
}

#[derive(Debug, Default)]
struct FakeServerState {
    bearer_tokens: Vec<String>,
    registered_devices: Vec<Device>,
    registered_services: Vec<Vec<Service>>,
    p2p_certificates: Vec<(DeviceId, Vec<u8>)>,
    listed_devices: Vec<DeviceId>,
    claimed_sessions: Vec<SessionId>,
    bound_sessions: Vec<SessionId>,
    closed_sessions: Vec<SessionId>,
}

impl FakeServerApi {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeServerState::default())),
        }
    }
}

#[async_trait]
impl ServerApi for FakeServerApi {
    fn set_bearer_token(&mut self, bearer_token: String) {
        self.state
            .lock()
            .expect("fake server state poisoned")
            .bearer_tokens
            .push(bearer_token);
    }

    async fn register_device(&mut self, device: Device) -> Result<(), ControlClientError> {
        self.state
            .lock()
            .expect("fake server state poisoned")
            .registered_devices
            .push(device);
        Ok(())
    }

    async fn register_services(
        &mut self,
        services: Vec<Service>,
    ) -> Result<(), ControlClientError> {
        self.state
            .lock()
            .expect("fake server state poisoned")
            .registered_services
            .push(services);
        Ok(())
    }

    async fn register_p2p_certificate(
        &mut self,
        device_id: &DeviceId,
        certificate_der: Vec<u8>,
    ) -> Result<(), ControlClientError> {
        self.state
            .lock()
            .expect("fake server state poisoned")
            .p2p_certificates
            .push((device_id.clone(), certificate_der));
        Ok(())
    }

    async fn list_agent_sessions(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<Vec<AgentSessionAssignment>, ControlClientError> {
        self.state
            .lock()
            .expect("fake server state poisoned")
            .listed_devices
            .push(device_id.clone());
        Ok(vec![assignment(AgentSessionStatus::Pending)])
    }

    async fn claim_agent_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        self.state
            .lock()
            .expect("fake server state poisoned")
            .claimed_sessions
            .push(session_id.clone());
        Ok(assignment(AgentSessionStatus::Claimed))
    }

    async fn mark_agent_session_bound(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        self.state
            .lock()
            .expect("fake server state poisoned")
            .bound_sessions
            .push(session_id.clone());
        Ok(assignment(AgentSessionStatus::Bound))
    }

    async fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        self.state
            .lock()
            .expect("fake server state poisoned")
            .closed_sessions
            .push(session_id.clone());
        Ok(assignment(AgentSessionStatus::Closed))
    }
}

#[tokio::test]
async fn server_sdk_registers_device_services_and_p2p_certificate_with_saved_credential() {
    let api = FakeServerApi::new();
    let sdk = ServerSdk::new(api.clone(), credential_store().await);

    sdk.register_device(device()).await.unwrap();
    sdk.register_services(vec![service()]).await.unwrap();
    sdk.register_p2p_certificate(vec![1, 2, 3]).await.unwrap();

    let state = api.state.lock().expect("fake server state poisoned");
    assert_eq!(
        state.bearer_tokens,
        vec![
            "server-token".to_string(),
            "server-token".to_string(),
            "server-token".to_string(),
        ]
    );
    assert_eq!(state.registered_devices, vec![device()]);
    assert_eq!(state.registered_services, vec![vec![service()]]);
    assert_eq!(
        state.p2p_certificates,
        vec![(DeviceId::new("pc_001"), vec![1, 2, 3])]
    );
}

#[tokio::test]
async fn server_sdk_register_server_runs_full_registration_workflow() {
    let api = FakeServerApi::new();
    let sdk = ServerSdk::new(api.clone(), credential_store().await);

    sdk.register_server(ServerRegistrationInput {
        device: device(),
        services: vec![service()],
        p2p_certificate_der: Some(vec![4, 5, 6]),
    })
    .await
    .unwrap();

    let state = api.state.lock().expect("fake server state poisoned");
    assert_eq!(state.registered_devices, vec![device()]);
    assert_eq!(
        state.p2p_certificates,
        vec![(DeviceId::new("pc_001"), vec![4, 5, 6])]
    );
    assert_eq!(state.registered_services, vec![vec![service()]]);
}

#[tokio::test]
async fn server_sdk_session_lifecycle_uses_saved_credential() {
    let api = FakeServerApi::new();
    let sdk = ServerSdk::new(api.clone(), credential_store().await);

    assert_eq!(
        sdk.list_sessions().await.unwrap(),
        vec![assignment(AgentSessionStatus::Pending)]
    );
    assert_eq!(
        sdk.claim_session(&SessionId::new("sess_001"))
            .await
            .unwrap(),
        assignment(AgentSessionStatus::Claimed)
    );
    assert_eq!(
        sdk.mark_session_bound(&SessionId::new("sess_001"))
            .await
            .unwrap(),
        assignment(AgentSessionStatus::Bound)
    );
    assert_eq!(
        sdk.close_session(&SessionId::new("sess_001"))
            .await
            .unwrap(),
        assignment(AgentSessionStatus::Closed)
    );

    let state = api.state.lock().expect("fake server state poisoned");
    assert_eq!(state.listed_devices, vec![DeviceId::new("pc_001")]);
    assert_eq!(state.claimed_sessions, vec![SessionId::new("sess_001")]);
    assert_eq!(state.bound_sessions, vec![SessionId::new("sess_001")]);
    assert_eq!(state.closed_sessions, vec![SessionId::new("sess_001")]);
    assert_eq!(
        state.bearer_tokens,
        vec![
            "server-token".to_string(),
            "server-token".to_string(),
            "server-token".to_string(),
            "server-token".to_string(),
        ]
    );
}

#[tokio::test]
async fn server_sdk_requires_saved_credential() {
    let api = FakeServerApi::new();
    let sdk = ServerSdk::new(api.clone(), MemoryServerCredentialStore::default());

    let err = sdk.list_sessions().await.unwrap_err();

    assert!(matches!(err, SdkError::NotAuthenticated));
    let state = api.state.lock().expect("fake server state poisoned");
    assert!(state.bearer_tokens.is_empty());
    assert!(state.listed_devices.is_empty());
}

#[tokio::test]
async fn server_sdk_loads_and_clears_credential() {
    let store = credential_store().await;
    let sdk = ServerSdk::new(FakeServerApi::new(), store.clone());

    assert_eq!(sdk.load_credential().await.unwrap(), Some(credential()));
    sdk.clear_credential().await.unwrap();
    assert_eq!(store.load_credential().await.unwrap(), None);
}

async fn credential_store() -> MemoryServerCredentialStore {
    let store = MemoryServerCredentialStore::default();
    store.save_credential(credential()).await.unwrap();
    store
}

fn credential() -> StoredServerCredential {
    StoredServerCredential {
        control_server: "http://control.local".to_string(),
        credential_id: "srv_cred_001".to_string(),
        device_id: DeviceId::new("pc_001"),
        device_name: "Office PC".to_string(),
        server_token: "server-token".to_string(),
        token_type: "Bearer".to_string(),
    }
}

fn device() -> Device {
    Device {
        device_id: DeviceId::new("pc_001"),
        user_id: UserId::new("user_001"),
        name: "Office PC".to_string(),
        status: DeviceStatus::Online,
        agent_version: "0.1.0".to_string(),
    }
}

fn service() -> Service {
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
        expire_at: 1000,
        status,
        grant_id: None,
        grant_revocation_version: None,
        grant_service_id: None,
    }
}
