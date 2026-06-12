use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use quic_tunnel_auth::{RelayTokenClaims, TokenKey, TokenSigner};
use quic_tunnel_protocol::{ClientId, DeviceId, ServiceId, SessionId, UserId};
use quic_tunnel_relay::{
    admin::{routes, RelayAdminHealth, RelayAdminSession},
    bind::{RelayBindRequest, RelayPeerRole, SharedKeyRelayTokenVerifier},
    session::RelaySessionStore,
};
use tower::ServiceExt;

fn claims(session_id: &str) -> RelayTokenClaims {
    RelayTokenClaims {
        session_id: SessionId::new(session_id),
        user_id: UserId::new("user_001"),
        client_id: ClientId::new("mobile_001"),
        device_id: DeviceId::new("pc_001"),
        service_id: ServiceId::new("svc_web_3000"),
        max_bps: 2_097_152,
        max_streams: 32,
        max_duration_sec: 3_600,
        traffic_quota_bytes: 1_073_741_824,
        exp: 4_102_444_800,
    }
}

fn store() -> (RelaySessionStore, TokenSigner) {
    let signer = TokenSigner::new(TokenKey::new("dev-secret"));
    let verifier = SharedKeyRelayTokenVerifier::new(TokenKey::new("dev-secret"), 1_767_000_000);
    (RelaySessionStore::new(Arc::new(verifier)), signer)
}

async fn request(app: axum::Router, method: Method, uri: &str) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get(app: axum::Router, uri: &str) -> axum::response::Response {
    request(app, Method::GET, uri).await
}

async fn post(app: axum::Router, uri: &str) -> axum::response::Response {
    request(app, Method::POST, uri).await
}

async fn json<T: serde::de::DeserializeOwned>(response: axum::response::Response) -> T {
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn text(response: axum::response::Response) -> String {
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}

#[tokio::test]
async fn admin_health_reports_session_metrics() {
    let (store, signer) = store();
    let session_id = SessionId::new("sess_health");
    let token = signer.sign_relay(&claims("sess_health")).unwrap();

    store
        .bind(RelayBindRequest {
            role: RelayPeerRole::Mobile,
            session_id: session_id.clone(),
            token: token.clone(),
        })
        .unwrap();
    store
        .bind(RelayBindRequest {
            role: RelayPeerRole::Agent,
            session_id: session_id.clone(),
            token,
        })
        .unwrap();
    let permit = store.begin_stream(&session_id).unwrap();
    store.add_traffic(&session_id, 5, 7).unwrap();

    let response = get(routes(store), "/admin/health").await;
    assert_eq!(response.status(), StatusCode::OK);
    let health: RelayAdminHealth = json(response).await;

    assert_eq!(health.status, "healthy");
    assert_eq!(health.active_sessions, 1);
    assert_eq!(health.active_streams, 1);
    assert_eq!(health.total_uplink_bytes, 5);
    assert_eq!(health.total_downlink_bytes, 7);
    assert_eq!(health.total_bytes, 12);

    drop(permit);
}

#[tokio::test]
async fn admin_routes_expose_relay_sessions_and_stats() {
    let (store, signer) = store();
    let session_id = SessionId::new("sess_001");
    let token = signer.sign_relay(&claims("sess_001")).unwrap();

    store
        .bind(RelayBindRequest {
            role: RelayPeerRole::Mobile,
            session_id: session_id.clone(),
            token: token.clone(),
        })
        .unwrap();
    store
        .bind(RelayBindRequest {
            role: RelayPeerRole::Agent,
            session_id: session_id.clone(),
            token,
        })
        .unwrap();
    store.add_traffic(&session_id, 5, 7).unwrap();

    let app = routes(store);
    let response = get(app.clone(), "/admin/sessions").await;
    assert_eq!(response.status(), StatusCode::OK);
    let sessions: Vec<RelayAdminSession> = json(response).await;

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, session_id);
    assert_eq!(sessions[0].state, "ready");
    assert!(sessions[0].mobile_bound);
    assert!(sessions[0].agent_bound);
    assert_eq!(sessions[0].stats.uplink_bytes, 5);
    assert_eq!(sessions[0].stats.downlink_bytes, 7);
    assert_eq!(sessions[0].stats.total_bytes, 12);

    let response = get(app.clone(), "/admin/sessions/sess_001").await;
    assert_eq!(response.status(), StatusCode::OK);
    let session: RelayAdminSession = json(response).await;
    assert_eq!(session.session_id, session_id);

    let response = post(app.clone(), "/admin/sessions/sess_001/disconnect").await;
    assert_eq!(response.status(), StatusCode::OK);
    let session: RelayAdminSession = json(response).await;
    assert_eq!(session.state, "closed");

    let response = post(app.clone(), "/admin/sessions/missing/disconnect").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = get(app, "/admin/sessions/missing").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_routes_serve_test_panel() {
    let (store, _) = store();
    let app = routes(store);

    let response = get(app, "/admin").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/html; charset=utf-8"
    );

    let body = text(response).await;
    assert!(body.contains("QUIC Relay Admin Test"));
    assert!(body.contains(r#"id="sessionList""#));
    assert!(body.contains("/admin/sessions"));
}
