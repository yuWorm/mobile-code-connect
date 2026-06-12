use std::sync::Arc;
use std::time::{Duration, Instant};

use mobilecode_connect_auth::{RelayTokenClaims, TokenKey, TokenSigner};
use mobilecode_connect_protocol::{ClientId, DeviceId, ServiceId, SessionId, UserId};
use mobilecode_connect_relay::{
    bind::{RelayBindRequest, RelayPeerRole, SharedKeyRelayTokenVerifier},
    limiter::RelayLimiter,
    session::{RelayError, RelaySessionState, RelaySessionStore},
};

fn claims(
    session_id: &str,
    max_streams: u32,
    max_duration_sec: u64,
    traffic_quota_bytes: u64,
) -> RelayTokenClaims {
    RelayTokenClaims {
        session_id: SessionId::new(session_id),
        user_id: UserId::new("user_001"),
        client_id: ClientId::new("mobile_001"),
        device_id: DeviceId::new("pc_001"),
        service_id: ServiceId::new("svc_web_3000"),
        max_bps: 2_097_152,
        max_streams,
        max_duration_sec,
        traffic_quota_bytes,
        exp: 4_102_444_800,
    }
}

fn store() -> (RelaySessionStore, TokenSigner) {
    let signer = TokenSigner::new(TokenKey::new("dev-secret"));
    let verifier = SharedKeyRelayTokenVerifier::new(TokenKey::new("dev-secret"), 1_767_000_000);
    (RelaySessionStore::new(Arc::new(verifier)), signer)
}

fn bind_ready(store: &RelaySessionStore, signer: &TokenSigner, claims: RelayTokenClaims) {
    let session_id = claims.session_id.clone();
    let token = signer.sign_relay(&claims).unwrap();
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
            session_id,
            token,
        })
        .unwrap();
}

#[test]
fn begin_stream_enforces_max_streams_and_releases_on_drop() {
    let (store, signer) = store();
    let session_id = SessionId::new("sess_001");
    bind_ready(&store, &signer, claims("sess_001", 1, 3_600, 1_024));

    let permit = store.begin_stream(&session_id).unwrap();
    let err = store.begin_stream(&session_id).unwrap_err();

    assert!(matches!(err, RelayError::MaxStreamsExceeded { .. }));
    assert_eq!(store.get(&session_id).unwrap().stats.active_streams, 1);

    drop(permit);

    assert_eq!(store.get(&session_id).unwrap().stats.active_streams, 0);
    assert!(store.begin_stream(&session_id).is_ok());
}

#[test]
fn stream_permit_exposes_session_bandwidth_limit() {
    let (store, signer) = store();
    let session_id = SessionId::new("sess_001");
    let mut token_claims = claims("sess_001", 1, 3_600, 1_024);
    token_claims.max_bps = 128;
    bind_ready(&store, &signer, token_claims);

    let permit = store.begin_stream(&session_id).unwrap();

    assert_eq!(permit.max_bps(), 128);
}

#[test]
fn stream_permits_share_session_bandwidth_limiter() {
    let (store, signer) = store();
    let session_id = SessionId::new("sess_001");
    let mut token_claims = claims("sess_001", 2, 3_600, 1_024);
    token_claims.max_bps = 4;
    bind_ready(&store, &signer, token_claims);

    let first = store.begin_stream(&session_id).unwrap();
    let second = store.begin_stream(&session_id).unwrap();
    let now = Instant::now();

    assert_eq!(first.limiter().reserve_delay_at(4, now), Duration::ZERO);
    assert_eq!(
        second.limiter().reserve_delay_at(1, now),
        Duration::from_millis(250)
    );
}

#[test]
fn relay_limiter_schedules_bytes_by_configured_bps() {
    let limiter = RelayLimiter::new(4);
    let now = Instant::now();

    assert_eq!(limiter.reserve_delay_at(2, now), Duration::ZERO);
    assert_eq!(limiter.reserve_delay_at(2, now), Duration::ZERO);
    assert_eq!(limiter.reserve_delay_at(1, now), Duration::from_millis(250));
    assert_eq!(
        limiter.reserve_delay_at(3, now + Duration::from_secs(1)),
        Duration::ZERO
    );
}

#[test]
fn relay_limiter_zero_bps_is_unlimited() {
    let limiter = RelayLimiter::new(0);

    assert_eq!(
        limiter.reserve_delay_at(usize::MAX, Instant::now()),
        Duration::ZERO
    );
}

#[test]
fn traffic_over_quota_closes_session_and_rejects_new_streams() {
    let (store, signer) = store();
    let session_id = SessionId::new("sess_001");
    bind_ready(&store, &signer, claims("sess_001", 8, 3_600, 10));

    store.add_traffic(&session_id, 5, 4).unwrap();
    let err = store.add_traffic(&session_id, 1, 1).unwrap_err();

    assert!(matches!(err, RelayError::TrafficQuotaExceeded { .. }));
    let session = store.get(&session_id).unwrap();
    assert_eq!(session.state, RelaySessionState::Closed);
    assert_eq!(session.stats.total_bytes, 11);
    assert!(matches!(
        store.begin_stream(&session_id).unwrap_err(),
        RelayError::SessionClosed { .. }
    ));
}

#[test]
fn expired_session_duration_closes_session_before_stream_begin() {
    let (store, signer) = store();
    let session_id = SessionId::new("sess_001");
    bind_ready(&store, &signer, claims("sess_001", 8, 0, 1_024));

    let err = store.begin_stream(&session_id).unwrap_err();

    assert!(matches!(err, RelayError::SessionExpired { .. }));
    assert_eq!(
        store.get(&session_id).unwrap().state,
        RelaySessionState::Closed
    );
}

#[test]
fn closed_session_rejects_binds_and_streams() {
    let (store, signer) = store();
    let session_id = SessionId::new("sess_001");
    let claims = claims("sess_001", 8, 3_600, 1_024);
    bind_ready(&store, &signer, claims.clone());

    let closed = store.close(&session_id).unwrap();

    assert_eq!(closed.state, RelaySessionState::Closed);
    assert!(matches!(
        store.begin_stream(&session_id).unwrap_err(),
        RelayError::SessionClosed { .. }
    ));
    assert!(matches!(
        store
            .bind(RelayBindRequest {
                role: RelayPeerRole::Mobile,
                session_id,
                token: signer.sign_relay(&claims).unwrap(),
            })
            .unwrap_err(),
        RelayError::SessionClosed { .. }
    ));
}
