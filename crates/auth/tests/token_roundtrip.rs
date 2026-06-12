use quic_tunnel_auth::{
    ControlRole, ControlTokenClaims, RelayTokenClaims, TokenError, TokenKey, TokenSigner,
};
use quic_tunnel_protocol::{ClientId, DeviceId, ServiceId, SessionId, UserId};

fn relay_claims(exp: u64) -> RelayTokenClaims {
    RelayTokenClaims {
        session_id: SessionId::new("sess_001"),
        user_id: UserId::new("user_001"),
        client_id: ClientId::new("mobile_001"),
        device_id: DeviceId::new("pc_001"),
        service_id: ServiceId::new("svc_web_3000"),
        max_bps: 2_097_152,
        max_streams: 32,
        max_duration_sec: 3_600,
        traffic_quota_bytes: 1_073_741_824,
        exp,
    }
}

#[test]
fn relay_token_roundtrips_with_limits() {
    let signer = TokenSigner::new(TokenKey::new("dev-secret"));
    let claims = relay_claims(4_102_444_800);

    let token = signer.sign_relay(&claims).unwrap();
    let decoded = signer.verify_relay(&token, 1_767_000_000).unwrap();

    assert_eq!(decoded, claims);
}

#[test]
fn expired_relay_token_is_rejected() {
    let signer = TokenSigner::new(TokenKey::new("dev-secret"));
    let token = signer.sign_relay(&relay_claims(1_000)).unwrap();

    let err = signer.verify_relay(&token, 1_001).unwrap_err();

    assert!(matches!(err, TokenError::Expired));
}

#[test]
fn tampered_relay_token_is_rejected() {
    let signer = TokenSigner::new(TokenKey::new("dev-secret"));
    let mut token = signer.sign_relay(&relay_claims(4_102_444_800)).unwrap();
    token.push('x');

    let err = signer.verify_relay(&token, 1_767_000_000).unwrap_err();

    assert!(matches!(err, TokenError::InvalidSignature));
}

#[test]
fn control_token_roundtrips_with_role() {
    let signer = TokenSigner::new(TokenKey::new("dev-secret"));
    let claims = ControlTokenClaims {
        user_id: UserId::new("admin_001"),
        subject: "admin@example.com".to_string(),
        role: ControlRole::Admin,
        exp: 4_102_444_800,
        relay_token_version: None,
        credential_id: None,
        server_credential_version: None,
    };

    let token = signer.sign_control(&claims).unwrap();
    let decoded = signer.verify_control(&token, 1_767_000_000).unwrap();

    assert_eq!(decoded, claims);
}

#[test]
fn agent_control_token_preserves_credential_metadata() {
    let signer = TokenSigner::new(TokenKey::new("dev-secret"));
    let claims = ControlTokenClaims {
        user_id: UserId::new("user_001"),
        subject: "srv_cred_001".to_string(),
        role: ControlRole::Agent,
        exp: 4_102_444_800,
        relay_token_version: None,
        credential_id: Some("srv_cred_001".to_string()),
        server_credential_version: Some(3),
    };

    let token = signer.sign_control(&claims).unwrap();
    let decoded = signer.verify_control(&token, 1_767_000_000).unwrap();

    assert_eq!(decoded, claims);
}
