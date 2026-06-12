use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use mobilecode_connect_control::{
    routes::routes,
    session::{CreateSessionRequest, CreateSessionResponse, RegisterP2pCertificateRequest},
    state::ControlState,
};
use mobilecode_connect_protocol::{
    Device, DeviceId, DeviceStatus, Service, ServiceId, ServiceProtocol, UserId,
};
use serde::de::DeserializeOwned;
use tower::ServiceExt;

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

async fn post_json<T: serde::Serialize>(
    app: axum::Router,
    uri: &str,
    payload: &T,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(payload).unwrap()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get(app: axum::Router, uri: &str) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn json<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn agent_registers_device_and_services_then_mobile_lists_them() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4433",
        "punch.example.com:3478",
    );
    let app = routes(state);

    let response = post_json(app.clone(), "/agent/register", &device()).await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = post_json(app.clone(), "/agent/services", &vec![service()]).await;
    assert_eq!(response.status(), StatusCode::OK);

    let devices: Vec<Device> = json(get(app.clone(), "/mobile/devices").await).await;
    let services: Vec<Service> = json(get(app, "/mobile/devices/pc_001/services").await).await;

    assert_eq!(devices, vec![device()]);
    assert_eq!(services, vec![service()]);
}

#[tokio::test]
async fn mobile_creates_session_and_receives_tokens_and_addrs() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4433",
        "punch.example.com:3478",
    );
    let app = routes(state);
    post_json(app.clone(), "/agent/register", &device()).await;
    post_json(app.clone(), "/agent/services", &vec![service()]).await;

    let response = post_json(
        app,
        "/sessions",
        &CreateSessionRequest {
            client_id: "mobile_001".to_string(),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
        },
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let created: CreateSessionResponse = json(response).await;
    assert_eq!(created.relay_addr, "relay.example.com:4433");
    assert_eq!(created.punch_addr, "punch.example.com:3478");
    assert!(!created.access_token.is_empty());
    assert!(!created.relay_token.is_empty());
    assert!(created.expire_at > 0);
}

#[tokio::test]
async fn mobile_session_includes_registered_agent_p2p_certificate() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4433",
        "punch.example.com:3478",
    );
    let app = routes(state);
    post_json(app.clone(), "/agent/register", &device()).await;
    post_json(app.clone(), "/agent/services", &vec![service()]).await;
    let cert_der = vec![1, 2, 3, 4];
    let response = post_json(
        app.clone(),
        "/agent/devices/pc_001/p2p-cert",
        &RegisterP2pCertificateRequest {
            certificate_der: cert_der.clone(),
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = post_json(
        app,
        "/sessions",
        &CreateSessionRequest {
            client_id: "mobile_001".to_string(),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
        },
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let created: CreateSessionResponse = json(response).await;
    assert_eq!(created.agent_p2p_cert_der, Some(cert_der));
}
