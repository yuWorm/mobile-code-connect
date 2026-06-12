use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use mobilecode_connect_agent::{
    config::{AgentConfig, ServiceConfig},
    runtime::{Agent, AgentControlRuntime, AgentControlRuntimeConfig, AgentP2pRuntimeConfig},
    service_registry::ServiceRegistry,
};
use mobilecode_connect_control::{routes::routes, state::ControlState};
use mobilecode_connect_control_client::HttpControlClientOptions;
use mobilecode_connect_mobile_core::{
    client::{OpenServiceRequest, TunnelClient},
    config::TunnelConfig,
    forward::{ControlP2pOrRelayConnectorConfig, ControlP2pOrRelayStreamConnector},
};
use mobilecode_connect_protocol::{ClientId, DeviceId, ServiceId, ServiceProtocol};
use mobilecode_connect_relay::{config::RelayConfig, runtime::RelayService};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

#[tokio::test]
async fn p2p_failure_falls_back_to_relay_from_control_session() {
    let relay = RelayService::new_quic(
        RelayConfig {
            token_secret: "dev-secret".to_string(),
            now_epoch_sec: 1_767_000_000,
        },
        local_addr(0),
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

    let dead_punch = unused_udp_addr().await;
    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control_addr = control.local_addr().unwrap();
    let control_url = format!("http://{control_addr}");
    let state = ControlState::new("dev-secret", relay_addr.to_string(), dead_punch.to_string());
    let control_task = tokio::spawn(async move {
        axum::serve(control, routes(state)).await.unwrap();
    });

    let service = service_config(echo_port);
    Agent::register_with_control(AgentConfig {
        device_id: DeviceId::new("pc_001"),
        control_server: control_url.clone(),
        auth_token: "agent-token".to_string(),
        services: vec![service.clone()],
        p2p_certificate_der: None,
    })
    .await
    .unwrap();
    let registry = ServiceRegistry::new(vec![service]).unwrap();
    let agent = AgentControlRuntime::new(AgentControlRuntimeConfig {
        control_server_url: control_url.clone(),
        auth_token: "agent-token".to_string(),
        device_id: DeviceId::new("pc_001"),
        relay_server_cert: relay_cert.clone(),
        registry,
        poll_interval: Duration::from_millis(10),
        p2p: Some(AgentP2pRuntimeConfig {
            bind_addr: local_addr(0),
            candidate_timeout: Duration::from_millis(50),
            probe_timeout: Duration::from_millis(50),
            interval: Duration::from_millis(10),
            server_identity: None,
        }),
        mobile_grants: None,
    })
    .unwrap();
    let (agent_shutdown_tx, agent_shutdown_rx) = oneshot::channel();
    let agent_task = tokio::spawn(agent.run_until(async {
        let _ = agent_shutdown_rx.await;
    }));

    let connector = ControlP2pOrRelayStreamConnector::new(ControlP2pOrRelayConnectorConfig {
        control_server_url: control_url.clone(),
        control_token: None,
        mobile_grant: None,
        client_id: ClientId::new("mobile_001"),
        control_client_options: HttpControlClientOptions::default(),
        relay_server_cert: relay_cert,
        bind_addr: local_addr(0),
        candidate_timeout: Duration::from_millis(50),
        probe_timeout: Duration::from_millis(50),
        interval: Duration::from_millis(10),
        relay_fallback_delay: Duration::from_millis(10),
    })
    .unwrap();
    let client = TunnelClient::with_connector(
        TunnelConfig {
            user_token: "user-token".to_string(),
            control_server_url: control_url,
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
    tokio::time::timeout(Duration::from_secs(2), local.read_exact(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&response, b"world");

    drop(local);
    client
        .close_service(handle.handle_id().to_string())
        .await
        .unwrap();

    let mut sessions = session_store.list();
    for _ in 0..50 {
        if sessions.iter().any(|session| session.stats.total_bytes > 0) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
        sessions = session_store.list();
    }
    let session = sessions
        .iter()
        .find(|session| session.mobile.is_some() && session.agent.is_some())
        .expect("fallback should bind a Relay session");
    assert_eq!(session.stats.uplink_bytes, 5);
    assert_eq!(session.stats.downlink_bytes, 5);
    assert_eq!(session.stats.total_bytes, 10);

    let _ = agent_shutdown_tx.send(());
    let _ = relay_shutdown_tx.send(());
    agent_task.await.unwrap().unwrap();
    relay_task.await.unwrap().unwrap();
    echo_task.await.unwrap();
    control_task.abort();
}

fn service_config(target_port: u16) -> ServiceConfig {
    ServiceConfig {
        service_id: ServiceId::new("svc_web_3000"),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port,
    }
}

async fn unused_udp_addr() -> SocketAddr {
    let socket = tokio::net::UdpSocket::bind(local_addr(0)).await.unwrap();
    socket.local_addr().unwrap()
}

fn local_addr(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
}
