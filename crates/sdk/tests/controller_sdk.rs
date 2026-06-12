use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use quic_tunnel_control_client::{
    ControlClientError, ControllerDevice, CreateSessionRequest, CreateSessionResponse,
    RegisterControllerDeviceRequest,
};
use quic_tunnel_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, Service, ServiceId, ServiceProtocol, SessionId,
    UserId,
};
use quic_tunnel_sdk::{
    controller::{ControllerApi, ControllerSdk, CreateSessionInput, RegisterControllerInput},
    store::{MemoryTokenStore, StoredToken, TokenStore},
    SdkError,
};

#[derive(Clone)]
struct FakeControllerApi {
    state: Arc<Mutex<FakeControllerState>>,
}

#[derive(Debug, Default)]
struct FakeControllerState {
    bearer_tokens: Vec<String>,
    register_requests: Vec<RegisterControllerDeviceRequest>,
    service_device_ids: Vec<DeviceId>,
    session_requests: Vec<CreateSessionRequest>,
}

impl FakeControllerApi {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeControllerState::default())),
        }
    }
}

#[async_trait]
impl ControllerApi for FakeControllerApi {
    fn set_bearer_token(&mut self, bearer_token: String) {
        self.state
            .lock()
            .expect("fake controller state poisoned")
            .bearer_tokens
            .push(bearer_token);
    }

    async fn register_controller(
        &mut self,
        request: RegisterControllerDeviceRequest,
    ) -> Result<ControllerDevice, ControlClientError> {
        self.state
            .lock()
            .expect("fake controller state poisoned")
            .register_requests
            .push(request);
        Ok(controller())
    }

    async fn list_devices(&mut self) -> Result<Vec<Device>, ControlClientError> {
        Ok(vec![device()])
    }

    async fn list_device_services(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<Vec<Service>, ControlClientError> {
        self.state
            .lock()
            .expect("fake controller state poisoned")
            .service_device_ids
            .push(device_id.clone());
        Ok(vec![service()])
    }

    async fn create_session(
        &mut self,
        request: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, ControlClientError> {
        self.state
            .lock()
            .expect("fake controller state poisoned")
            .session_requests
            .push(request);
        Ok(session())
    }
}

#[tokio::test]
async fn register_controller_uses_saved_token() {
    let fake = FakeControllerApi::new();
    let sdk = ControllerSdk::new(fake.clone(), token_store().await);

    let registered = sdk
        .register_controller(RegisterControllerInput {
            client_id: ClientId::new("phone_001"),
            name: "Phone".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(registered, controller());
    let state = fake.state.lock().expect("fake controller state poisoned");
    assert_eq!(state.bearer_tokens, vec!["token.saved".to_string()]);
    assert_eq!(
        state.register_requests,
        vec![RegisterControllerDeviceRequest {
            client_id: "phone_001".to_string(),
            name: "Phone".to_string(),
        }]
    );
}

#[tokio::test]
async fn list_devices_and_services_use_saved_token() {
    let fake = FakeControllerApi::new();
    let sdk = ControllerSdk::new(fake.clone(), token_store().await);

    assert_eq!(sdk.list_devices().await.unwrap(), vec![device()]);
    assert_eq!(
        sdk.list_device_services(&DeviceId::new("pc_001"))
            .await
            .unwrap(),
        vec![service()]
    );

    let state = fake.state.lock().expect("fake controller state poisoned");
    assert_eq!(
        state.bearer_tokens,
        vec!["token.saved".to_string(), "token.saved".to_string()]
    );
    assert_eq!(state.service_device_ids, vec![DeviceId::new("pc_001")]);
}

#[tokio::test]
async fn create_session_uses_client_device_and_service_ids() {
    let fake = FakeControllerApi::new();
    let sdk = ControllerSdk::new(fake.clone(), token_store().await);

    let created = sdk
        .create_session(CreateSessionInput {
            client_id: ClientId::new("phone_001"),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web"),
        })
        .await
        .unwrap();

    assert_eq!(created, session());
    let state = fake.state.lock().expect("fake controller state poisoned");
    assert_eq!(state.bearer_tokens, vec!["token.saved".to_string()]);
    assert_eq!(
        state.session_requests,
        vec![CreateSessionRequest {
            client_id: "phone_001".to_string(),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web"),
        }]
    );
}

#[tokio::test]
async fn controller_sdk_requires_saved_token() {
    let sdk = ControllerSdk::new(FakeControllerApi::new(), MemoryTokenStore::default());

    let err = sdk.list_devices().await.unwrap_err();

    assert!(matches!(err, SdkError::NotAuthenticated));
}

async fn token_store() -> MemoryTokenStore {
    let store = MemoryTokenStore::default();
    store
        .save_token(StoredToken {
            user_id: UserId::new("user_001"),
            access_token: "token.saved".to_string(),
            expire_at: 100,
        })
        .await
        .unwrap();
    store
}

fn controller() -> ControllerDevice {
    ControllerDevice {
        user_id: UserId::new("user_001"),
        client_id: ClientId::new("phone_001"),
        name: "Phone".to_string(),
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

fn session() -> CreateSessionResponse {
    CreateSessionResponse {
        session_id: SessionId::new("sess_001"),
        access_token: "access-token".to_string(),
        relay_token: "relay-token".to_string(),
        relay_addr: "127.0.0.1:4443".to_string(),
        punch_addr: "127.0.0.1:3478".to_string(),
        agent_p2p_cert_der: Some(vec![1, 2, 3]),
        expire_at: 1000,
    }
}
