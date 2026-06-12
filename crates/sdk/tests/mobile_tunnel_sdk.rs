use mobilecode_connect_control::{routes::routes, state::ControlState};
use mobilecode_connect_control_client::ApproveMobilePairingRequest;
use mobilecode_connect_protocol::{
    derive_mobile_grant_secret, ClientId, Device, DeviceId, DeviceStatus, MobileGrantCredential,
    MobileInvitePayload, ServiceId, UserId,
};
use mobilecode_connect_sdk::{
    mobile::{
        BrowserProxyConfig, BrowserProxyDirectFallbackPolicy, BrowserProxyRouteKind,
        BrowserProxyUrlKind, MobileTunnelConfig, MobileTunnelSdk, OpenServiceInput,
        P2pOrRelayTunnelConfig,
    },
    store::{MemoryTokenStore, StoredToken, TokenStore},
    HttpControlClientOptions, MemoryMobileGrantStore, MobileGrantPairingInput, MobileGrantStore,
    SdkError, TunnelPath,
};
use rustls::pki_types::CertificateDer;
use std::{net::SocketAddr, time::Duration};
use tokio::net::TcpListener;

#[tokio::test]
async fn mobile_tunnel_sdk_starts_with_saved_token_and_opens_ephemeral_forward() {
    let store = token_store().await;
    let tunnel = MobileTunnelSdk::start_in_memory(config(), store)
        .await
        .unwrap();

    assert_eq!(tunnel.status().active_forwards, 0);
    let handle = tunnel
        .open_service(OpenServiceInput {
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web"),
            local_port: 0,
        })
        .await
        .unwrap();

    assert_ne!(handle.local_port(), 0);
    assert_eq!(tunnel.status().active_forwards, 1);
    tunnel.close_service(handle.handle_id()).await.unwrap();
    assert_eq!(tunnel.status().active_forwards, 0);
}

#[tokio::test]
async fn mobile_tunnel_sdk_requires_saved_token() {
    let err = MobileTunnelSdk::start_in_memory(config(), MemoryTokenStore::default())
        .await
        .unwrap_err();

    assert!(matches!(err, SdkError::NotAuthenticated));
}

#[tokio::test]
async fn mobile_tunnel_sdk_starts_browser_proxy_and_builds_device_service_route() {
    let store = token_store().await;
    let tunnel = MobileTunnelSdk::start_in_memory(config(), store)
        .await
        .unwrap();

    let proxy = tunnel
        .start_browser_proxy(BrowserProxyConfig::default())
        .await
        .unwrap();
    assert_eq!(proxy.host(), "127.0.0.1");
    assert_ne!(proxy.local_port(), 0);
    let stats = proxy.stats();
    assert_eq!(stats.accepted_connections, 0);
    assert_eq!(stats.active_connections, 0);
    assert_eq!(stats.direct_connections, 0);
    assert_eq!(stats.tunnel_connections, 0);
    assert_eq!(stats.tunnel_bytes_to_remote, 0);
    assert_eq!(stats.tunnel_bytes_to_browser, 0);
    assert_eq!(stats.direct_bytes_to_remote, 0);
    assert_eq!(stats.direct_bytes_to_browser, 0);
    assert_eq!(stats.idle_timeout_closures, 0);

    let route = tunnel
        .browser_proxy_device_service_route(DeviceId::new("pc_001"), ServiceId::new("svc_web"))
        .unwrap();
    assert_eq!(route.kind, BrowserProxyRouteKind::DeviceService);
    assert_eq!(route.device_id.as_str(), "pc_001");
    assert_eq!(route.service_id.as_str(), "svc_web");
    assert_eq!(
        route.http_url("/status"),
        format!("http://{}/status", route.host)
    );
    assert_eq!(
        route.http_url("status"),
        format!("http://{}/status", route.host)
    );

    let device = tunnel
        .classify_browser_proxy_url(route.http_url("/status?q=1"))
        .unwrap();
    assert_eq!(device.kind, BrowserProxyUrlKind::DeviceService);
    let target = device.target.expect("device service target");
    assert_eq!(target.device_id.as_str(), "pc_001");
    assert_eq!(target.service_id.as_str(), "svc_web");

    let control = tunnel
        .classify_browser_proxy_url("http://127.0.0.1:4242/devices")
        .unwrap();
    assert_eq!(control.kind, BrowserProxyUrlKind::ControlServer);
    assert!(control.target.is_none());

    let direct = tunnel
        .classify_browser_proxy_url("https://example.com/app.js")
        .unwrap();
    assert_eq!(direct.kind, BrowserProxyUrlKind::DirectNetwork);

    tunnel.close_browser_proxy(proxy).await.unwrap();
}

#[tokio::test]
async fn mobile_tunnel_sdk_starts_with_mobile_grant_without_saved_user_token() {
    let grant = MobileGrantCredential {
        version: 1,
        control_url: config().control_server_url,
        device_id: DeviceId::new("pc_001"),
        grant_id: "gr_mobile_001".to_string(),
        client_id: ClientId::new("phone_001"),
        allowed_services: vec![ServiceId::new("svc_web")],
        grant_secret: "grant-secret".to_string(),
        revocation_version: 1,
        agent_p2p_cert_fingerprint: None,
    };
    let tunnel = MobileTunnelSdk::start_with_mobile_grant(
        config(),
        grant,
        P2pOrRelayTunnelConfig {
            relay_server_cert: CertificateDer::from(vec![1, 2, 3]),
            bind_addr: "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
            candidate_timeout: Duration::from_millis(20),
            probe_timeout: Duration::from_millis(20),
            interval: Duration::from_millis(10),
            relay_fallback_delay: Duration::from_millis(5),
        },
    )
    .await
    .unwrap();

    assert_eq!(tunnel.status().path, TunnelPath::P2p);
}

#[tokio::test]
async fn mobile_tunnel_sdk_pairs_mobile_grant_and_stores_credential() {
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", "127.0.0.1:3478");
    state
        .register_device(Device {
            device_id: DeviceId::new("pc_pairing"),
            user_id: UserId::new("user_001"),
            name: "Pairing PC".to_string(),
            status: DeviceStatus::Online,
            agent_version: "0.1.0".to_string(),
        })
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(state)).await.unwrap();
    });
    let control_url = format!("http://{addr}");
    let client_id = ClientId::new("phone_pairing");
    let service_id = ServiceId::new("svc_web");
    let invite = MobileInvitePayload {
        version: 1,
        control_url: control_url.clone(),
        device_id: DeviceId::new("pc_pairing"),
        invite_id: "inv_pairing".to_string(),
        invite_secret: "invite-secret".to_string(),
        agent_p2p_cert_fingerprint: None,
        allowed_services: vec![service_id.clone()],
        expires_at: 4_102_444_800,
        max_uses: 1,
    };
    let store = MemoryMobileGrantStore::default();

    let pairing = MobileTunnelSdk::start_mobile_grant_pairing(
        MobileGrantPairingInput {
            invite: invite.clone(),
            client_id: client_id.clone(),
            requested_services: vec![service_id.clone()],
            nonce: "pairing-nonce".to_string(),
        },
        HttpControlClientOptions::default(),
    )
    .await
    .unwrap();

    let agent = mobilecode_connect_control_client::HttpControlClient::new(control_url).unwrap();
    let approved = agent
        .approve_mobile_pairing(
            &pairing.pending_pairing_id,
            ApproveMobilePairingRequest {
                grant_id: "gr_pairing".to_string(),
                allowed_services: vec![service_id.clone()],
                revocation_version: 1,
            },
        )
        .await
        .unwrap();
    assert!(approved.grant.is_some());

    let grant = MobileTunnelSdk::complete_mobile_grant_pairing_once(
        pairing,
        store.clone(),
        HttpControlClientOptions::default(),
    )
    .await
    .unwrap()
    .expect("approved grant");
    assert_eq!(grant.grant_id, "gr_pairing");
    assert_eq!(
        grant.grant_secret,
        derive_mobile_grant_secret("invite-secret", "gr_pairing", &client_id).unwrap()
    );
    assert_eq!(store.load_mobile_grant().await.unwrap(), Some(grant));

    server.abort();
}

#[test]
fn browser_proxy_config_defaults_include_mobile_safe_timeouts() {
    let config = BrowserProxyConfig::default();

    assert_eq!(config.bind_host, "127.0.0.1");
    assert_eq!(config.local_port, 0);
    assert_eq!(config.domain_suffix, ".mobilecode-connect.local");
    assert_eq!(config.max_connections, 256);
    assert_eq!(
        config.direct_fallback_policy,
        BrowserProxyDirectFallbackPolicy::LocalNetworkAndDomain
    );
    assert_eq!(config.request_head_timeout, Duration::from_secs(10));
    assert_eq!(config.direct_connect_timeout, Duration::from_secs(10));
    assert_eq!(config.tunnel_open_timeout, Duration::from_secs(15));
    assert_eq!(config.idle_timeout, Duration::from_secs(120));
}

async fn token_store() -> MemoryTokenStore {
    let store = MemoryTokenStore::default();
    store
        .save_token(StoredToken {
            user_id: UserId::new("user_001"),
            access_token: "token.saved".to_string(),
            expire_at: 100,
        })
        .await
        .unwrap();
    store
}

fn config() -> MobileTunnelConfig {
    MobileTunnelConfig {
        control_server_url: "http://127.0.0.1:4242".to_string(),
        client_id: ClientId::new("phone_001"),
        control_client_options: HttpControlClientOptions::default(),
    }
}
