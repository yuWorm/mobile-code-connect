use quic_tunnel_control::{routes::routes, state::ControlState};
use quic_tunnel_control_client::{
    AgentSessionStatus, ControlClientError, CreateSessionRequest, HttpControlClient, Plan,
    RegisterControllerDeviceRequest, RegisterRelayRequest, RegisterUserRequest, UpdateRelayRequest,
    UpdateUserPlanRequest,
};
use quic_tunnel_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, RelayLimits, Service, ServiceId, ServiceProtocol,
    UserId,
};
use tokio::net::TcpListener;

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
        service_id: ServiceId::new("svc_web_3000"),
        device_id: DeviceId::new("pc_001"),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: 3000,
    }
}

#[tokio::test]
async fn http_client_registers_agent_and_creates_mobile_session() {
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });

    let client = HttpControlClient::new(format!("http://{addr}")).unwrap();
    client.register_device(device()).await.unwrap();
    client.register_services(vec![service()]).await.unwrap();

    let devices = client.list_devices().await.unwrap();
    let services = client
        .list_device_services(&DeviceId::new("pc_001"))
        .await
        .unwrap();
    let session = client
        .create_session(CreateSessionRequest {
            client_id: "mobile_001".to_string(),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
        })
        .await
        .unwrap();
    let agent_sessions = client
        .list_agent_sessions(&DeviceId::new("pc_001"))
        .await
        .unwrap();

    assert_eq!(devices, vec![device()]);
    assert_eq!(services, vec![service()]);
    assert_eq!(session.relay_addr, "127.0.0.1:4443");
    assert_eq!(session.punch_addr, "127.0.0.1:3478");
    assert!(!session.access_token.is_empty());
    assert!(!session.relay_token.is_empty());
    assert_eq!(agent_sessions.len(), 1);
    assert_eq!(agent_sessions[0].session_id, session.session_id);
    assert_eq!(agent_sessions[0].device_id, DeviceId::new("pc_001"));
    assert_eq!(agent_sessions[0].service_id, ServiceId::new("svc_web_3000"));
    assert_eq!(agent_sessions[0].client_id, ClientId::new("mobile_001"));
    assert_eq!(agent_sessions[0].relay_addr, "127.0.0.1:4443");
    assert_eq!(agent_sessions[0].punch_addr, "127.0.0.1:3478");
    assert_eq!(agent_sessions[0].relay_token, session.relay_token);
    assert_eq!(agent_sessions[0].status, AgentSessionStatus::Pending);

    let claimed = client
        .claim_agent_session(&session.session_id)
        .await
        .unwrap();
    assert_eq!(claimed.status, AgentSessionStatus::Claimed);

    let pending_after_claim = client
        .list_agent_sessions(&DeviceId::new("pc_001"))
        .await
        .unwrap();
    assert!(pending_after_claim.is_empty());

    let second_claim = client.claim_agent_session(&session.session_id).await;
    assert!(matches!(
        second_claim,
        Err(ControlClientError::HttpStatus { status_code, .. }) if status_code.as_u16() == 409
    ));

    let bound = client
        .mark_agent_session_bound(&session.session_id)
        .await
        .unwrap();
    assert_eq!(bound.status, AgentSessionStatus::Bound);

    let closed = client.close_session(&session.session_id).await.unwrap();
    assert_eq!(closed.status, AgentSessionStatus::Closed);

    server.abort();
}

#[tokio::test]
async fn http_client_updates_and_reads_user_plan() {
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    let admin_token = state.issue_admin_token("admin@example.com").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let control_url = format!("http://{addr}");
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });

    let mut client = HttpControlClient::new(control_url.clone()).unwrap();
    let admin_client = HttpControlClient::with_bearer_token(control_url, admin_token).unwrap();
    let auth = client
        .register_user(RegisterUserRequest {
            email: "plan-client@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Plan Client".to_string(),
        })
        .await
        .unwrap();
    client.set_bearer_token(auth.access_token);
    let plan = Plan {
        plan_id: "team".to_string(),
        name: "Team".to_string(),
        max_controller_devices: 4,
        relay_limits: RelayLimits {
            max_bps: 8_192,
            max_streams: 12,
            max_duration_sec: 3_600,
            traffic_quota_bytes: 2_097_152,
        },
    };

    let updated = admin_client
        .update_user_plan(&auth.user_id, UpdateUserPlanRequest { plan: plan.clone() })
        .await
        .unwrap();
    let fetched = admin_client.user_plan(&auth.user_id).await.unwrap();
    let current = client.current_plan().await.unwrap();

    assert_eq!(updated, plan);
    assert_eq!(fetched, plan);
    assert_eq!(current, plan);

    server.abort();
}

#[tokio::test]
async fn http_client_lists_and_removes_controllers() {
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });

    let mut client = HttpControlClient::new(format!("http://{addr}")).unwrap();
    let auth = client
        .register_user(RegisterUserRequest {
            email: "controller-client@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Controller Client".to_string(),
        })
        .await
        .unwrap();
    client.set_bearer_token(auth.access_token);
    for client_id in ["phone_001", "laptop_001"] {
        client
            .register_controller(RegisterControllerDeviceRequest {
                client_id: client_id.to_string(),
                name: client_id.to_string(),
            })
            .await
            .unwrap();
    }

    let controllers = client.list_controllers().await.unwrap();
    assert_eq!(controllers.items.len(), 2);

    client
        .remove_controller(&ClientId::new("phone_001"))
        .await
        .unwrap();
    let controllers = client.list_controllers().await.unwrap();
    assert_eq!(controllers.items.len(), 1);
    assert_eq!(controllers.items[0].client_id.as_str(), "laptop_001");

    server.abort();
}

#[tokio::test]
async fn http_client_lists_gets_and_removes_controlled_devices() {
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });

    let client = HttpControlClient::new(format!("http://{addr}")).unwrap();
    client.register_device(device()).await.unwrap();
    client.register_services(vec![service()]).await.unwrap();

    let devices = client.list_controlled_devices().await.unwrap();
    assert_eq!(devices.items, vec![device()]);
    assert_eq!(
        client
            .controlled_device(&DeviceId::new("pc_001"))
            .await
            .unwrap(),
        device()
    );

    client
        .remove_controlled_device(&DeviceId::new("pc_001"))
        .await
        .unwrap();
    let devices = client.list_controlled_devices().await.unwrap();
    assert!(devices.items.is_empty());

    server.abort();
}

#[tokio::test]
async fn http_client_updates_and_removes_relays() {
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    let admin_token = state.issue_admin_token("admin@example.com").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let control_url = format!("http://{addr}");
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });

    let admin_client = HttpControlClient::with_bearer_token(control_url, admin_token).unwrap();
    admin_client
        .register_relay(RegisterRelayRequest {
            relay_id: "relay_ops".to_string(),
            relay_addr: "relay.example.com:4443".to_string(),
            admin_addr: "relay.example.com:9090".to_string(),
            capacity_streams: 16,
        })
        .await
        .unwrap();

    let updated = admin_client
        .update_relay(
            "relay_ops",
            UpdateRelayRequest {
                relay_addr: "relay-new.example.com:4443".to_string(),
                admin_addr: "relay-new.example.com:9090".to_string(),
                capacity_streams: 32,
                healthy: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.relay_addr, "relay-new.example.com:4443");
    assert_eq!(updated.capacity_streams, 32);
    assert!(!updated.healthy);
    assert_eq!(admin_client.relay("relay_ops").await.unwrap(), updated);

    admin_client.remove_relay("relay_ops").await.unwrap();
    let relays = admin_client.list_relays().await.unwrap();
    assert!(!relays
        .items
        .iter()
        .any(|relay| relay.relay_id == "relay_ops"));

    server.abort();
}
