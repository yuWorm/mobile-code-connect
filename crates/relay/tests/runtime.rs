use quic_tunnel_auth::{RelayTokenClaims, TokenKey, TokenSigner};
use quic_tunnel_protocol::{ClientId, DeviceId, ServiceId, SessionId, UserId};
use quic_tunnel_relay::{
    bind::{RelayBindRequest, RelayBindStatus, RelayPeerRole},
    config::RelayConfig,
    runtime::RelayService,
};

fn claims() -> RelayTokenClaims {
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
        exp: 4_102_444_800,
    }
}

#[tokio::test]
async fn relay_service_exposes_embeddable_session_store() {
    let service = RelayService::new(RelayConfig {
        token_secret: "dev-secret".to_string(),
        now_epoch_sec: 1_767_000_000,
    })
    .await
    .unwrap();
    let signer = TokenSigner::new(TokenKey::new("dev-secret"));
    let token = signer.sign_relay(&claims()).unwrap();

    let status = service
        .session_store()
        .bind(RelayBindRequest {
            role: RelayPeerRole::Mobile,
            session_id: SessionId::new("sess_001"),
            token,
        })
        .unwrap();

    assert_eq!(status, RelayBindStatus::Waiting);
}

#[tokio::test]
async fn relay_service_run_until_returns_after_shutdown() {
    let service = RelayService::new(RelayConfig {
        token_secret: "dev-secret".to_string(),
        now_epoch_sec: 1_767_000_000,
    })
    .await
    .unwrap();

    service.run_until(async {}).await.unwrap();
}
