use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use quic_tunnel_agent::{
    config::ServiceConfig,
    relay_client::{RelayAgentClient, RelayAgentConfig},
    service_registry::ServiceRegistry,
};
use quic_tunnel_control::{routes::routes, state::ControlState};
use quic_tunnel_control_client::{
    CreateSessionRequest, HttpControlClient, HttpControlClientOptions,
};
use quic_tunnel_mobile_core::{
    client::{OpenServiceRequest, TunnelClient},
    config::TunnelConfig,
    forward::{RelayConnectorConfig, RelayStreamConnector},
};
use quic_tunnel_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, Service, ServiceId, ServiceProtocol, UserId,
};
use quic_tunnel_relay::{config::RelayConfig, runtime::RelayService};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

fn device() -> Device {
    Device {
        device_id: DeviceId::new("pc_001"),
        user_id: UserId::new("user_001"),
        name: "Office PC".to_string(),
        status: DeviceStatus::Online,
        agent_version: "0.1.0".to_string(),
    }
}

fn service(target_port: u16) -> Service {
    Service {
        service_id: ServiceId::new("svc_web_3000"),
        device_id: DeviceId::new("pc_001"),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port,
    }
}

#[tokio::test]
async fn control_created_session_token_drives_relay_tunnel() {
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
    let state = ControlState::new("dev-secret", relay_addr.to_string(), "127.0.0.1:3478");
    let control_task = tokio::spawn(async move {
        axum::serve(control, routes(state)).await.unwrap();
    });
    let control_client = HttpControlClient::new(format!("http://{control_addr}")).unwrap();
    control_client.register_device(device()).await.unwrap();
    control_client
        .register_services(vec![service(echo_port)])
        .await
        .unwrap();
    let session = control_client
        .create_session(CreateSessionRequest {
            client_id: "mobile_001".to_string(),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
        })
        .await
        .unwrap();

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
        session_id: session.session_id.clone(),
        token: session.relay_token.clone(),
        registry,
    })
    .await
    .unwrap();
    let (agent_shutdown_tx, agent_shutdown_rx) = oneshot::channel();
    let agent_task = tokio::spawn(agent.run_until(async {
        let _ = agent_shutdown_rx.await;
    }));

    let connector = RelayStreamConnector::connect(RelayConnectorConfig {
        relay_addr: session.relay_addr.parse().unwrap(),
        server_cert: relay_cert,
        session_id: session.session_id,
        token: session.relay_token,
    })
    .await
    .unwrap();
    let client = TunnelClient::with_connector(
        TunnelConfig {
            user_token: "user-token".to_string(),
            control_server_url: format!("http://{control_addr}"),
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
    let _ = relay_shutdown_tx.send(());
    agent_task.await.unwrap().unwrap();
    relay_task.await.unwrap().unwrap();
    echo_task.await.unwrap();
    control_task.abort();
}
