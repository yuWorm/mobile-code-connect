use std::{net::SocketAddr, time::Duration};

use quic_tunnel_control_client::HttpControlClientOptions;
use quic_tunnel_mobile_core::{
    client::{ControlP2pOrRelayClientConfig, OpenServiceRequest, TunnelClient, TunnelError},
    config::TunnelConfig,
    path::{PathSelector, TunnelPath},
};
use quic_tunnel_protocol::{ClientId, DeviceId, ServiceId};
use rustls::pki_types::CertificateDer;

fn config() -> TunnelConfig {
    TunnelConfig {
        user_token: "user-token".to_string(),
        control_server_url: "http://127.0.0.1:4242".to_string(),
        client_id: ClientId::new("mobile_001"),
        control_client_options: HttpControlClientOptions::default(),
    }
}

fn open_request(local_port: u16) -> OpenServiceRequest {
    OpenServiceRequest {
        device_id: DeviceId::new("pc_001"),
        service_id: ServiceId::new("svc_web_3000"),
        local_port,
    }
}

#[test]
fn tunnel_config_rejects_empty_control_url() {
    let mut config = config();
    config.control_server_url.clear();

    let err = config.validate().unwrap_err();

    assert!(matches!(err, TunnelError::InvalidConfig { .. }));
}

#[test]
fn tunnel_config_carries_control_client_timeout_and_retry_options() {
    let options = HttpControlClientOptions::default()
        .with_request_timeout(Duration::from_secs(5))
        .with_max_retries(2)
        .with_retry_backoff(Duration::from_millis(25));
    let mut config = config();
    config.control_client_options = options;

    assert_eq!(config.control_client_options, options);
    config.validate().unwrap();
}

#[test]
fn default_path_selector_uses_relay_for_phase_one() {
    assert_eq!(PathSelector::default().select(), TunnelPath::Relay);
}

#[test]
fn path_selector_prefers_p2p_when_probe_is_ready() {
    assert_eq!(
        PathSelector::default().select_with_probe_result(true),
        TunnelPath::P2p
    );
    assert_eq!(
        PathSelector::default().select_with_probe_result(false),
        TunnelPath::Relay
    );
}

#[tokio::test]
async fn tunnel_client_tracks_open_and_closed_forward_handles() {
    let client = TunnelClient::start(config()).await.unwrap();
    let port_probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_port = port_probe.local_addr().unwrap().port();
    drop(port_probe);

    let handle = client.open_service(open_request(local_port)).await.unwrap();
    assert_eq!(handle.local_port(), local_port);
    assert_eq!(client.status().active_forwards, 1);

    client
        .close_service(handle.handle_id().to_string())
        .await
        .unwrap();

    assert_eq!(client.status().active_forwards, 0);
}

#[tokio::test]
async fn open_service_allows_zero_local_port_for_ephemeral_bind() {
    let client = TunnelClient::start(config()).await.unwrap();
    let handle = client.open_service(open_request(0)).await.unwrap();

    assert_ne!(handle.local_port(), 0);
    client
        .close_service(handle.handle_id().to_string())
        .await
        .unwrap();
}

#[tokio::test]
async fn tunnel_client_start_with_control_keeps_relay_only_api() {
    let client = TunnelClient::start_with_control(config(), CertificateDer::from(vec![1, 2, 3]))
        .await
        .unwrap();

    assert_eq!(client.config().client_id.as_str(), "mobile_001");
    assert_eq!(client.status().path, TunnelPath::Relay);
}

#[tokio::test]
async fn tunnel_client_start_with_control_p2p_or_relay_uses_sdk_config() {
    let client = TunnelClient::start_with_control_p2p_or_relay(
        config(),
        ControlP2pOrRelayClientConfig {
            relay_server_cert: CertificateDer::from(vec![1, 2, 3]),
            bind_addr: SocketAddr::from(([0, 0, 0, 0], 0)),
            candidate_timeout: Duration::from_millis(1500),
            probe_timeout: Duration::from_millis(1500),
            interval: Duration::from_millis(25),
            relay_fallback_delay: Duration::from_millis(300),
        },
    )
    .await
    .unwrap();

    assert_eq!(client.config().client_id.as_str(), "mobile_001");
    let status = client.status();
    assert_eq!(status.path, TunnelPath::P2p);
    assert_eq!(status.transport.p2p_attempts, 0);
    assert_eq!(status.transport.p2p_connections, 0);
    assert_eq!(status.transport.p2p_failures, 0);
    assert_eq!(status.transport.relay_fallbacks, 0);
    assert_eq!(status.transport.relay_connections, 0);
    assert_eq!(status.transport.relay_failures, 0);
    assert_eq!(status.transport.last_successful_path, None);
}
