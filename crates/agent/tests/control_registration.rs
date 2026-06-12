use mobilecode_connect_agent::{
    config::{AgentConfig, ServiceConfig},
    mobile_grant::{CreateMobileInviteRequest, MobileGrantManager},
    runtime::{Agent, AgentControlRuntime, AgentControlRuntimeConfig},
    service_registry::ServiceRegistry,
};
use mobilecode_connect_control::{routes::routes, state::ControlState};
use mobilecode_connect_control_client::{
    AgentSessionAssignment, AgentSessionStatus, CreateSessionRequest, HttpControlClient,
    RegisterUserRequest,
};
use mobilecode_connect_protocol::{
    derive_mobile_grant_secret, ClientId, Device, DeviceId, DeviceStatus, MobileGrantCredential,
    MobilePairingRequest, PendingGrantSessionStatus, PendingPairingStatus, Service, ServiceId,
    ServiceProtocol, SessionId, UserId,
};
use rustls::pki_types::CertificateDer;
use tokio::net::TcpListener;

fn service() -> ServiceConfig {
    ServiceConfig {
        service_id: ServiceId::new("svc_web_3000"),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: 3000,
    }
}

fn control_device(device_id: &str) -> Device {
    Device {
        device_id: DeviceId::new(device_id),
        user_id: UserId::new("user_001"),
        name: "Office PC".to_string(),
        status: DeviceStatus::Online,
        agent_version: "0.1.0".to_string(),
    }
}

fn control_service(device_id: &str, service_id: &str) -> Service {
    Service {
        service_id: ServiceId::new(service_id),
        device_id: DeviceId::new(device_id),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: 3000,
    }
}

#[tokio::test]
async fn agent_registers_configured_device_and_services_with_control() {
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });

    let control_server = format!("http://{addr}");
    Agent::register_with_control(AgentConfig {
        device_id: DeviceId::new("pc_001"),
        control_server: control_server.clone(),
        auth_token: "agent-token".to_string(),
        services: vec![service()],
        p2p_certificate_der: None,
    })
    .await
    .unwrap();

    let client = HttpControlClient::new(control_server).unwrap();
    let devices = client.list_devices().await.unwrap();
    let services = client
        .list_device_services(&DeviceId::new("pc_001"))
        .await
        .unwrap();

    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device_id, DeviceId::new("pc_001"));
    assert_eq!(devices[0].user_id, UserId::new("user_001"));
    assert_eq!(devices[0].status, DeviceStatus::Online);
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].device_id, DeviceId::new("pc_001"));
    assert_eq!(services[0].service_id, ServiceId::new("svc_web_3000"));
    assert_eq!(services[0].target_port, 3000);

    server.abort();
}

#[tokio::test]
async fn agent_registers_p2p_certificate_with_control() {
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });

    let control_server = format!("http://{addr}");
    let p2p_cert = vec![9, 8, 7, 6];
    Agent::register_with_control(AgentConfig {
        device_id: DeviceId::new("pc_001"),
        control_server: control_server.clone(),
        auth_token: "agent-token".to_string(),
        services: vec![service()],
        p2p_certificate_der: Some(p2p_cert.clone()),
    })
    .await
    .unwrap();

    let client = HttpControlClient::new(control_server).unwrap();
    let session = client
        .create_session(CreateSessionRequest {
            client_id: "mobile_001".to_string(),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
        })
        .await
        .unwrap();

    assert_eq!(session.agent_p2p_cert_der, Some(p2p_cert));

    server.abort();
}

#[tokio::test]
async fn agent_control_token_registers_device_for_authenticated_user() {
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });

    let control_server = format!("http://{addr}");
    let bootstrap = HttpControlClient::new(control_server.clone()).unwrap();
    let auth = bootstrap
        .register_user(RegisterUserRequest {
            email: "owner@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Owner".to_string(),
        })
        .await
        .unwrap();

    Agent::register_with_control(AgentConfig {
        device_id: DeviceId::new("server_001"),
        control_server: control_server.clone(),
        auth_token: auth.access_token.clone(),
        services: vec![service()],
        p2p_certificate_der: None,
    })
    .await
    .unwrap();

    let authed_client =
        HttpControlClient::with_bearer_token(control_server, auth.access_token).unwrap();
    let devices = authed_client.list_devices().await.unwrap();
    let services = authed_client
        .list_device_services(&DeviceId::new("server_001"))
        .await
        .unwrap();

    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device_id, DeviceId::new("server_001"));
    assert_eq!(devices[0].user_id, auth.user_id);
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].device_id, DeviceId::new("server_001"));

    server.abort();
}

#[tokio::test]
async fn agent_runtime_refuses_revoked_mobile_grant_assignment_before_binding() {
    let device_id = DeviceId::new("pc_grant_runtime");
    let service_id = ServiceId::new("svc_web_3000");
    let client_id = ClientId::new("mobile_001");
    let grants = MobileGrantManager::default();
    let invite = grants
        .create_invite(
            CreateMobileInviteRequest {
                control_url: "http://127.0.0.1".to_string(),
                device_id: device_id.clone(),
                allowed_services: vec![service_id.clone()],
                ttl_sec: 4_102_444_800,
                max_uses: 1,
                agent_p2p_cert_fingerprint: None,
            },
            1_000,
        )
        .unwrap();
    let proof = MobilePairingRequest::proof_for(
        device_id.clone(),
        invite.invite_id.clone(),
        client_id.clone(),
        vec![service_id.clone()],
        "pairing-nonce".to_string(),
        &invite.invite_secret,
    )
    .unwrap();
    let pairing = MobilePairingRequest {
        device_id: device_id.clone(),
        invite_id: invite.invite_id,
        client_id: client_id.clone(),
        requested_services: vec![service_id.clone()],
        nonce: "pairing-nonce".to_string(),
        proof,
    };
    let grant = grants.approve_pairing(&pairing, 1_001).unwrap();
    grants.revoke_grant(&grant.grant_id).unwrap();

    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    state
        .register_device(control_device("pc_grant_runtime"))
        .unwrap();
    state
        .register_services(vec![control_service("pc_grant_runtime", "svc_web_3000")])
        .unwrap();
    let session_id = SessionId::new("sess_revoked_grant");
    state
        .add_agent_session(AgentSessionAssignment {
            session_id: session_id.clone(),
            user_id: UserId::new("user_001"),
            device_id: device_id.clone(),
            service_id: service_id.clone(),
            client_id: client_id.clone(),
            relay_token: "relay-token".to_string(),
            relay_addr: "127.0.0.1:4443".to_string(),
            punch_addr: "127.0.0.1:3478".to_string(),
            expire_at: 4_102_444_800,
            status: AgentSessionStatus::Pending,
            grant_id: Some(grant.grant_id.clone()),
            grant_revocation_version: Some(1),
            grant_service_id: Some(service_id.clone()),
        })
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });
    let control_server = format!("http://{addr}");
    let mut runtime = AgentControlRuntime::new(AgentControlRuntimeConfig {
        control_server_url: control_server.clone(),
        auth_token: "agent-token".to_string(),
        device_id: device_id.clone(),
        relay_server_cert: CertificateDer::from(Vec::new()),
        registry: ServiceRegistry::new(vec![service()]).unwrap(),
        poll_interval: std::time::Duration::from_millis(10),
        p2p: None,
        mobile_grants: Some(grants),
    })
    .unwrap();

    let started = runtime.poll_once().await.unwrap();
    assert!(started.is_empty());
    let client = HttpControlClient::new(control_server).unwrap();
    let pending = client.list_agent_sessions(&device_id).await.unwrap();
    assert!(pending.is_empty());

    server.abort();
}

#[tokio::test]
async fn agent_runtime_approves_mobile_pairing_and_grant_session_requests() {
    let device_id = DeviceId::new("pc_runtime_grant_approval");
    let service_id = ServiceId::new("svc_web_3000");
    let client_id = ClientId::new("mobile_001");
    let grants = MobileGrantManager::default();
    let invite = grants
        .create_invite(
            CreateMobileInviteRequest {
                control_url: "http://127.0.0.1".to_string(),
                device_id: device_id.clone(),
                allowed_services: vec![service_id.clone()],
                ttl_sec: 4_102_444_800,
                max_uses: 1,
                agent_p2p_cert_fingerprint: None,
            },
            1_000,
        )
        .unwrap();
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    state
        .register_device(control_device("pc_runtime_grant_approval"))
        .unwrap();
    state
        .register_services(vec![control_service(
            "pc_runtime_grant_approval",
            "svc_web_3000",
        )])
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });
    let control_server = format!("http://{addr}");
    let control = HttpControlClient::new(control_server.clone()).unwrap();

    let proof = MobilePairingRequest::proof_for(
        device_id.clone(),
        invite.invite_id.clone(),
        client_id.clone(),
        vec![service_id.clone()],
        "pairing-nonce".to_string(),
        &invite.invite_secret,
    )
    .unwrap();
    let pairing_started = control
        .start_mobile_pairing(MobilePairingRequest {
            device_id: device_id.clone(),
            invite_id: invite.invite_id.clone(),
            client_id: client_id.clone(),
            requested_services: vec![service_id.clone()],
            nonce: "pairing-nonce".to_string(),
            proof,
        })
        .await
        .unwrap();

    let mut runtime = AgentControlRuntime::new(AgentControlRuntimeConfig {
        control_server_url: control_server.clone(),
        auth_token: "agent-token".to_string(),
        device_id: device_id.clone(),
        relay_server_cert: CertificateDer::from(Vec::new()),
        registry: ServiceRegistry::new(vec![service()]).unwrap(),
        poll_interval: std::time::Duration::from_millis(10),
        p2p: None,
        mobile_grants: Some(grants.clone()),
    })
    .unwrap();

    assert!(runtime.poll_once().await.unwrap().is_empty());
    let pairing = control
        .mobile_pairing_result(&pairing_started.pending_pairing_id)
        .await
        .unwrap();
    assert_eq!(pairing.status, PendingPairingStatus::Approved);
    let metadata = pairing.grant.expect("approved grant metadata");
    let mobile_grant = MobileGrantCredential {
        version: metadata.version,
        control_url: control_server.clone(),
        device_id: metadata.device_id,
        grant_id: metadata.grant_id.clone(),
        client_id: metadata.client_id,
        allowed_services: metadata.allowed_services,
        grant_secret: derive_mobile_grant_secret(
            &invite.invite_secret,
            metadata.grant_id,
            &client_id,
        )
        .unwrap(),
        revocation_version: metadata.revocation_version,
        agent_p2p_cert_fingerprint: None,
    };
    let grant_session = control
        .start_grant_session(
            mobile_grant
                .sign_session_request(service_id.clone(), "session-nonce".to_string())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(runtime.poll_once().await.unwrap().is_empty());
    let result = control
        .grant_session_result(&grant_session.pending_session_id)
        .await
        .unwrap();
    assert_eq!(result.status, PendingGrantSessionStatus::Approved);
    assert!(result.session.is_some());

    server.abort();
}
