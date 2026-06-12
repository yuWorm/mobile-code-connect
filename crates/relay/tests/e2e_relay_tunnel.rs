use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use quic_tunnel_agent::{
    config::ServiceConfig,
    relay_client::{RelayAgentClient, RelayAgentConfig},
    service_registry::ServiceRegistry,
};
use quic_tunnel_auth::{RelayTokenClaims, TokenKey, TokenSigner};
use quic_tunnel_control_client::HttpControlClientOptions;
use quic_tunnel_mobile_core::{
    client::{OpenServiceRequest, TunnelClient},
    config::TunnelConfig,
    forward::{RelayConnectorConfig, RelayStreamConnector},
};
use quic_tunnel_protocol::{ClientId, DeviceId, ServiceId, ServiceProtocol, SessionId, UserId};
use quic_tunnel_relay::{config::RelayConfig, runtime::RelayService};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    time::{sleep, Duration},
};

fn relay_token(session_id: &SessionId) -> String {
    let signer = TokenSigner::new(TokenKey::new("dev-secret"));
    signer
        .sign_relay(&RelayTokenClaims {
            session_id: session_id.clone(),
            user_id: UserId::new("user_001"),
            client_id: ClientId::new("mobile_001"),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
            max_bps: 2_097_152,
            max_streams: 32,
            max_duration_sec: 3_600,
            traffic_quota_bytes: 1_073_741_824,
            exp: 4_102_444_800,
        })
        .unwrap()
}

#[tokio::test]
async fn mobile_local_port_reaches_agent_local_service_through_relay_quic() {
    let session_id = SessionId::new("sess_001");
    let token = relay_token(&session_id);

    let relay = RelayService::new_quic(
        RelayConfig {
            token_secret: "dev-secret".to_string(),
            now_epoch_sec: 1_767_000_000,
        },
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    )
    .await
    .unwrap();
    let relay_addr = relay.local_addr().unwrap();
    let relay_cert = relay.certificate_der().unwrap().clone();
    let session_store = relay.session_store();
    let (relay_shutdown_tx, relay_shutdown_rx) = oneshot::channel();
    let relay_task = tokio::spawn(relay.run_until(async {
        let _ = relay_shutdown_rx.await;
    }));

    let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_port = echo.local_addr().unwrap().port();
    let echo_task = tokio::spawn(async move {
        let (mut stream, _) = echo.accept().await.unwrap();
        let mut payload = [0_u8; 5];
        stream.read_exact(&mut payload).await.unwrap();
        stream.write_all(b"world").await.unwrap();
    });

    let registry = ServiceRegistry::new(vec![ServiceConfig {
        service_id: ServiceId::new("svc_web_3000"),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: echo_port,
    }])
    .unwrap();
    let agent = RelayAgentClient::connect(RelayAgentConfig {
        relay_addr,
        server_cert: relay_cert.clone(),
        session_id: session_id.clone(),
        token: token.clone(),
        registry,
    })
    .await
    .unwrap();
    let (agent_shutdown_tx, agent_shutdown_rx) = oneshot::channel();
    let agent_task = tokio::spawn(agent.run_until(async {
        let _ = agent_shutdown_rx.await;
    }));

    let connector = RelayStreamConnector::connect(RelayConnectorConfig {
        relay_addr,
        server_cert: relay_cert,
        session_id: session_id.clone(),
        token,
    })
    .await
    .unwrap();
    let client = TunnelClient::with_connector(
        TunnelConfig {
            user_token: "user-token".to_string(),
            control_server_url: "http://127.0.0.1:4242".to_string(),
            client_id: ClientId::new("mobile_001"),
            control_client_options: HttpControlClientOptions::default(),
        },
        Arc::new(connector),
    )
    .await
    .unwrap();

    let port_probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_port = port_probe.local_addr().unwrap().port();
    drop(port_probe);
    let handle = client
        .open_service(OpenServiceRequest {
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
            local_port,
        })
        .await
        .unwrap();

    let mut local = TcpStream::connect(("127.0.0.1", handle.local_port()))
        .await
        .unwrap();
    local.write_all(b"hello").await.unwrap();
    let mut response = [0_u8; 5];
    local.read_exact(&mut response).await.unwrap();

    assert_eq!(&response, b"world");

    drop(local);
    client
        .close_service(handle.handle_id().to_string())
        .await
        .unwrap();

    let mut session = session_store.get(&session_id).unwrap();
    for _ in 0..50 {
        if session.stats.total_bytes > 0 {
            break;
        }
        sleep(Duration::from_millis(20)).await;
        session = session_store.get(&session_id).unwrap();
    }
    assert_eq!(session.stats.uplink_bytes, 5);
    assert_eq!(session.stats.downlink_bytes, 5);
    assert_eq!(session.stats.total_bytes, 10);

    let _ = agent_shutdown_tx.send(());
    let _ = relay_shutdown_tx.send(());
    agent_task.await.unwrap().unwrap();
    relay_task.await.unwrap().unwrap();
    echo_task.await.unwrap();
}
