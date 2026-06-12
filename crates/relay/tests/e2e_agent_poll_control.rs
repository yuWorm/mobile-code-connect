use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use quic_tunnel_agent::{
    config::{AgentConfig, ServiceConfig},
    runtime::{Agent, AgentControlRuntime, AgentControlRuntimeConfig},
    service_registry::ServiceRegistry,
};
use quic_tunnel_control::{routes::routes, state::ControlState};
use quic_tunnel_control_client::{ControlClientError, HttpControlClient, HttpControlClientOptions};
use quic_tunnel_mobile_core::{
    client::{OpenServiceRequest, TunnelClient},
    config::TunnelConfig,
    forward::{ControlRelayConnectorConfig, ControlRelayStreamConnector},
};
use quic_tunnel_protocol::{ClientId, DeviceId, ServiceId, ServiceProtocol};
use quic_tunnel_relay::{config::RelayConfig, runtime::RelayService};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

fn service_config(target_port: u16) -> ServiceConfig {
    ServiceConfig {
        service_id: ServiceId::new("svc_web_3000"),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port,
    }
}

#[tokio::test]
async fn agent_polls_control_for_session_and_binds_relay_without_manual_token() {
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

    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control_addr = control.local_addr().unwrap();
    let control_url = format!("http://{control_addr}");
    let state = ControlState::new("dev-secret", relay_addr.to_string(), "127.0.0.1:3478");
    let control_task = tokio::spawn(async move {
        axum::serve(control, routes(state)).await.unwrap();
    });
    let control_probe = HttpControlClient::new(control_url.clone()).unwrap();

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
    let mut agent = AgentControlRuntime::new(AgentControlRuntimeConfig {
        control_server_url: control_url.clone(),
        auth_token: "agent-token".to_string(),
        device_id: DeviceId::new("pc_001"),
        relay_server_cert: relay_cert.clone(),
        registry,
        poll_interval: Duration::from_millis(20),
        p2p: None,
        mobile_grants: None,
    })
    .unwrap();

    let connector = ControlRelayStreamConnector::new(ControlRelayConnectorConfig {
        control_server_url: control_url.clone(),
        control_token: None,
        client_id: ClientId::new("mobile_001"),
        control_client_options: HttpControlClientOptions::default(),
        relay_server_cert: relay_cert,
    })
    .unwrap();
    let client = TunnelClient::with_connector(
        TunnelConfig {
            user_token: "user-token".to_string(),
            control_server_url: control_url.clone(),
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

    let mut started = Vec::new();
    for _ in 0..50 {
        started = agent.poll_once().await.unwrap();
        if !started.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(started.len(), 1);
    assert!(agent.poll_once().await.unwrap().is_empty());
    let second_claim = control_probe.claim_agent_session(&started[0]).await;
    assert!(matches!(
        second_claim,
        Err(ControlClientError::HttpStatus { status_code, .. }) if status_code.as_u16() == 409
    ));

    let mut response = [0_u8; 5];
    tokio::time::timeout(Duration::from_secs(2), local.read_exact(&mut response))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(&response, b"world");

    client
        .close_service(handle.handle_id().to_string())
        .await
        .unwrap();
    agent.shutdown().await;
    let close_again = control_probe.close_session(&started[0]).await;
    assert!(matches!(
        close_again,
        Err(ControlClientError::HttpStatus { status_code, .. }) if status_code.as_u16() == 409
    ));
    let _ = relay_shutdown_tx.send(());
    relay_task.await.unwrap().unwrap();
    echo_task.await.unwrap();
    control_task.abort();
}
