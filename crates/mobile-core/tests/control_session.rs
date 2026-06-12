use quic_tunnel_auth::{TokenKey, TokenSigner};
use quic_tunnel_control::{routes::routes, state::ControlState};
use quic_tunnel_control_client::{
    HttpControlClient, HttpControlClientOptions, RegisterUserRequest,
};
use quic_tunnel_mobile_core::forward::{
    ControlRelayConnectorConfig, ControlRelayStreamConnector, OpenForwardRequest,
};
use quic_tunnel_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, Service, ServiceId, ServiceProtocol, UserId,
};
use rustls::pki_types::CertificateDer;
use tokio::net::TcpListener;

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

fn device() -> Device {
    Device {
        device_id: DeviceId::new("server_001"),
        user_id: UserId::new("ignored"),
        name: "Server".to_string(),
        status: DeviceStatus::Online,
        agent_version: "0.1.0".to_string(),
    }
}

#[tokio::test]
async fn control_relay_connector_creates_session_and_builds_relay_config() {
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    state.register_services(vec![service()]).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });

    let connector = ControlRelayStreamConnector::new(ControlRelayConnectorConfig {
        control_server_url: format!("http://{addr}"),
        control_token: None,
        client_id: ClientId::new("mobile_001"),
        control_client_options: HttpControlClientOptions::default(),
        relay_server_cert: CertificateDer::from(vec![1, 2, 3]),
    })
    .unwrap();
    let relay = connector
        .resolve_relay_config(&OpenForwardRequest {
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
            local_port: 18080,
        })
        .await
        .unwrap();

    assert_eq!(relay.relay_addr.to_string(), "127.0.0.1:4443");
    assert_eq!(relay.session_id.as_str().starts_with("sess_"), true);
    assert!(!relay.token.is_empty());

    server.abort();
}

#[tokio::test]
async fn control_relay_connector_sends_control_token_when_creating_session() {
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });

    let control_url = format!("http://{addr}");
    let bootstrap = HttpControlClient::new(control_url.clone()).unwrap();
    let auth = bootstrap
        .register_user(RegisterUserRequest {
            email: "mobile-owner@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Owner".to_string(),
        })
        .await
        .unwrap();
    let authed =
        HttpControlClient::with_bearer_token(control_url.clone(), auth.access_token.clone())
            .unwrap();
    authed.register_device(device()).await.unwrap();
    authed
        .register_services(vec![Service {
            device_id: DeviceId::new("server_001"),
            ..service()
        }])
        .await
        .unwrap();

    let connector = ControlRelayStreamConnector::new(ControlRelayConnectorConfig {
        control_server_url: control_url,
        control_token: Some(auth.access_token),
        client_id: ClientId::new("phone_001"),
        control_client_options: HttpControlClientOptions::default(),
        relay_server_cert: CertificateDer::from(vec![1, 2, 3]),
    })
    .unwrap();
    let relay = connector
        .resolve_relay_config(&OpenForwardRequest {
            device_id: DeviceId::new("server_001"),
            service_id: ServiceId::new("svc_web_3000"),
            local_port: 18080,
        })
        .await
        .unwrap();

    let claims = TokenSigner::new(TokenKey::new("dev-secret"))
        .verify_relay(&relay.token, 1_767_000_000)
        .unwrap();
    assert_eq!(claims.user_id, auth.user_id);

    server.abort();
}
