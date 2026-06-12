use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use mobilecode_connect_agent::{
    config::ServiceConfig, p2p_client::P2pAgentClient, service_registry::ServiceRegistry,
};
use mobilecode_connect_control_client::HttpControlClientOptions;
use mobilecode_connect_mobile_core::{
    client::{OpenServiceRequest, TunnelClient},
    config::TunnelConfig,
    forward::P2pStreamConnector,
};
use mobilecode_connect_protocol::{
    ClientId, DeviceId, PeerRole, ServiceId, ServiceProtocol, SessionId,
};
use mobilecode_connect_punch::{
    probe::{establish_p2p_path, P2pPathConfig},
    server::PunchServer,
};
use mobilecode_connect_tunnel::quic::generate_self_signed_server_identity;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

#[tokio::test]
async fn mobile_local_port_reaches_agent_local_service_through_p2p_quic() {
    let punch = PunchServer::bind(local_addr(0)).await.unwrap();
    let punch_addr = punch.local_addr().unwrap();
    let (punch_shutdown_tx, punch_shutdown_rx) = oneshot::channel();
    let punch_task = tokio::spawn(punch.run_until(async {
        let _ = punch_shutdown_rx.await;
    }));

    let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_port = echo.local_addr().unwrap().port();
    let echo_task = tokio::spawn(async move {
        let (mut stream, _) = echo.accept().await.unwrap();
        let mut payload = [0_u8; 5];
        stream.read_exact(&mut payload).await.unwrap();
        stream.write_all(b"world").await.unwrap();
    });

    let session_id = SessionId::new("sess_p2p_001");
    let (mobile_path, agent_path) = tokio::try_join!(
        establish_p2p_path(p2p_config(
            session_id.clone(),
            PeerRole::Mobile,
            "mobile_001",
            "pc_001",
            punch_addr,
        )),
        establish_p2p_path(p2p_config(
            session_id.clone(),
            PeerRole::Agent,
            "pc_001",
            "mobile_001",
            punch_addr,
        ))
    )
    .unwrap();

    let registry = ServiceRegistry::new(vec![ServiceConfig {
        service_id: ServiceId::new("svc_web_3000"),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: echo_port,
    }])
    .unwrap();
    let agent = P2pAgentClient::from_path(agent_path, registry)
        .await
        .unwrap();
    let (agent_shutdown_tx, agent_shutdown_rx) = oneshot::channel();
    let agent_task = tokio::spawn(agent.run_until(async {
        let _ = agent_shutdown_rx.await;
    }));

    let connector = P2pStreamConnector::connect_path(session_id, mobile_path)
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

    client
        .close_service(handle.handle_id().to_string())
        .await
        .unwrap();
    let _ = agent_shutdown_tx.send(());
    let _ = punch_shutdown_tx.send(());
    agent_task.await.unwrap().unwrap();
    punch_task.await.unwrap().unwrap();
    echo_task.await.unwrap();
}

#[tokio::test]
async fn mobile_p2p_connector_verifies_agent_pinned_certificate() {
    let punch = PunchServer::bind(local_addr(0)).await.unwrap();
    let punch_addr = punch.local_addr().unwrap();
    let (punch_shutdown_tx, punch_shutdown_rx) = oneshot::channel();
    let punch_task = tokio::spawn(punch.run_until(async {
        let _ = punch_shutdown_rx.await;
    }));

    let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_port = echo.local_addr().unwrap().port();
    let echo_task = tokio::spawn(async move {
        let (mut stream, _) = echo.accept().await.unwrap();
        let mut payload = [0_u8; 5];
        stream.read_exact(&mut payload).await.unwrap();
        stream.write_all(b"world").await.unwrap();
    });

    let session_id = SessionId::new("sess_p2p_cert_001");
    let (mobile_path, agent_path) = tokio::try_join!(
        establish_p2p_path(p2p_config(
            session_id.clone(),
            PeerRole::Mobile,
            "mobile_001",
            "pc_001",
            punch_addr,
        )),
        establish_p2p_path(p2p_config(
            session_id.clone(),
            PeerRole::Agent,
            "pc_001",
            "mobile_001",
            punch_addr,
        ))
    )
    .unwrap();

    let identity = generate_self_signed_server_identity().unwrap();
    let agent_cert = identity.certificate_der().clone();
    let registry = ServiceRegistry::new(vec![ServiceConfig {
        service_id: ServiceId::new("svc_web_3000"),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: echo_port,
    }])
    .unwrap();
    let agent = P2pAgentClient::from_path_with_identity(agent_path, registry, identity)
        .await
        .unwrap();
    let (agent_shutdown_tx, agent_shutdown_rx) = oneshot::channel();
    let agent_task = tokio::spawn(agent.run_until(async {
        let _ = agent_shutdown_rx.await;
    }));

    let connector =
        P2pStreamConnector::connect_path_with_server_cert(session_id, mobile_path, agent_cert)
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

    client
        .close_service(handle.handle_id().to_string())
        .await
        .unwrap();
    let _ = agent_shutdown_tx.send(());
    let _ = punch_shutdown_tx.send(());
    agent_task.await.unwrap().unwrap();
    punch_task.await.unwrap().unwrap();
    echo_task.await.unwrap();
}

#[tokio::test]
async fn mobile_p2p_connector_rejects_unpinned_agent_certificate() {
    let punch = PunchServer::bind(local_addr(0)).await.unwrap();
    let punch_addr = punch.local_addr().unwrap();
    let (punch_shutdown_tx, punch_shutdown_rx) = oneshot::channel();
    let punch_task = tokio::spawn(punch.run_until(async {
        let _ = punch_shutdown_rx.await;
    }));

    let session_id = SessionId::new("sess_p2p_wrong_cert_001");
    let (mobile_path, agent_path) = tokio::try_join!(
        establish_p2p_path(p2p_config(
            session_id.clone(),
            PeerRole::Mobile,
            "mobile_001",
            "pc_001",
            punch_addr,
        )),
        establish_p2p_path(p2p_config(
            session_id.clone(),
            PeerRole::Agent,
            "pc_001",
            "mobile_001",
            punch_addr,
        ))
    )
    .unwrap();

    let agent_identity = generate_self_signed_server_identity().unwrap();
    let wrong_cert = generate_self_signed_server_identity()
        .unwrap()
        .certificate_der()
        .clone();
    let registry = ServiceRegistry::new(Vec::new()).unwrap();
    let agent = P2pAgentClient::from_path_with_identity(agent_path, registry, agent_identity)
        .await
        .unwrap();
    let (agent_shutdown_tx, agent_shutdown_rx) = oneshot::channel();
    let agent_task = tokio::spawn(agent.run_until(async {
        let _ = agent_shutdown_rx.await;
    }));

    let connector =
        P2pStreamConnector::connect_path_with_server_cert(session_id, mobile_path, wrong_cert)
            .await;

    assert!(connector.is_err());

    let _ = agent_shutdown_tx.send(());
    let _ = punch_shutdown_tx.send(());
    agent_task.await.unwrap().unwrap();
    punch_task.await.unwrap().unwrap();
}

fn p2p_config(
    session_id: SessionId,
    role: PeerRole,
    self_id: &str,
    peer_id: &str,
    punch_addr: SocketAddr,
) -> P2pPathConfig {
    P2pPathConfig {
        session_id,
        role,
        self_id: self_id.to_string(),
        peer_id: peer_id.to_string(),
        bind_addr: local_addr(0),
        punch_addr,
        shared_secret: "relay-token".to_string(),
        candidate_timeout: Duration::from_secs(1),
        probe_timeout: Duration::from_secs(1),
        interval: Duration::from_millis(10),
    }
}

fn local_addr(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
}
