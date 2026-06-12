use std::sync::Arc;

use quic_tunnel_auth::{RelayTokenClaims, TokenKey, TokenSigner};
use quic_tunnel_protocol::{ClientId, DeviceId, ServiceId, SessionId, UserId};
use quic_tunnel_relay::{
    bind::{RelayBindRequest, RelayBindStatus, RelayPeerRole, SharedKeyRelayTokenVerifier},
    session::{RelayError, RelaySessionState, RelaySessionStore},
};

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

fn token(signer: &TokenSigner, session_id: &str) -> String {
    signer.sign_relay(&claims(session_id)).unwrap()
}

fn store() -> (RelaySessionStore, TokenSigner) {
    let signer = TokenSigner::new(TokenKey::new("dev-secret"));
    let verifier = SharedKeyRelayTokenVerifier::new(TokenKey::new("dev-secret"), 1_767_000_000);
    (RelaySessionStore::new(Arc::new(verifier)), signer)
}

#[test]
fn mobile_bind_alone_leaves_session_waiting() {
    let (store, signer) = store();
    let session_id = SessionId::new("sess_001");

    let status = store
        .bind(RelayBindRequest {
            role: RelayPeerRole::Mobile,
            session_id: session_id.clone(),
            token: token(&signer, "sess_001"),
        })
        .unwrap();

    let session = store.get(&session_id).unwrap();
    assert_eq!(status, RelayBindStatus::Waiting);
    assert_eq!(session.state, RelaySessionState::Waiting);
    assert!(session.mobile.is_some());
    assert!(session.agent.is_none());
}

#[test]
fn agent_bind_alone_leaves_session_waiting() {
    let (store, signer) = store();
    let session_id = SessionId::new("sess_001");

    let status = store
        .bind(RelayBindRequest {
            role: RelayPeerRole::Agent,
            session_id: session_id.clone(),
            token: token(&signer, "sess_001"),
        })
        .unwrap();

    let session = store.get(&session_id).unwrap();
    assert_eq!(status, RelayBindStatus::Waiting);
    assert_eq!(session.state, RelaySessionState::Waiting);
    assert!(session.mobile.is_none());
    assert!(session.agent.is_some());
}

#[test]
fn mobile_and_agent_bind_marks_session_ready() {
    let (store, signer) = store();
    let session_id = SessionId::new("sess_001");

    store
        .bind(RelayBindRequest {
            role: RelayPeerRole::Mobile,
            session_id: session_id.clone(),
            token: token(&signer, "sess_001"),
        })
        .unwrap();
    let status = store
        .bind(RelayBindRequest {
            role: RelayPeerRole::Agent,
            session_id: session_id.clone(),
            token: token(&signer, "sess_001"),
        })
        .unwrap();

    let session = store.get(&session_id).unwrap();
    assert_eq!(status, RelayBindStatus::Ready);
    assert_eq!(session.state, RelaySessionState::Ready);
}

#[test]
fn duplicate_role_bind_is_rejected() {
    let (store, signer) = store();
    let request = RelayBindRequest {
        role: RelayPeerRole::Mobile,
        session_id: SessionId::new("sess_001"),
        token: token(&signer, "sess_001"),
    };

    store.bind(request.clone()).unwrap();
    let err = store.bind(request).unwrap_err();

    assert!(matches!(err, RelayError::DuplicateRole { .. }));
}

#[test]
fn mismatched_token_session_is_rejected() {
    let (store, signer) = store();

    let err = store
        .bind(RelayBindRequest {
            role: RelayPeerRole::Mobile,
            session_id: SessionId::new("sess_requested"),
            token: token(&signer, "sess_token"),
        })
        .unwrap_err();

    assert!(matches!(err, RelayError::SessionMismatch { .. }));
}
