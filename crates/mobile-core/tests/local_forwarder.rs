use std::{sync::Arc, time::Duration};

use quic_tunnel_mobile_core::forward::{LocalForwarder, MemoryStreamConnector, OpenForwardRequest};
use quic_tunnel_protocol::{DeviceId, ServiceId};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

fn request() -> OpenForwardRequest {
    OpenForwardRequest {
        device_id: DeviceId::new("pc_001"),
        service_id: ServiceId::new("svc_web_3000"),
        local_port: 0,
    }
}

#[tokio::test]
async fn local_forwarder_proxies_tcp_connection_through_connector_stream() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let forwarder = LocalForwarder::bind(request(), connector.clone())
        .await
        .unwrap();
    let mut local = TcpStream::connect(("127.0.0.1", forwarder.local_port()))
        .await
        .unwrap();
    let mut remote = connector.accept().await.unwrap();

    local.write_all(b"ping").await.unwrap();
    let mut remote_payload = [0_u8; 4];
    remote.read_exact(&mut remote_payload).await.unwrap();
    remote.write_all(b"pong").await.unwrap();

    let mut local_payload = [0_u8; 4];
    local.read_exact(&mut local_payload).await.unwrap();

    assert_eq!(&remote_payload, b"ping");
    assert_eq!(&local_payload, b"pong");

    forwarder.shutdown().await.unwrap();
}

#[tokio::test]
async fn local_forwarder_shutdown_stops_accepting_connections() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let forwarder = LocalForwarder::bind(request(), connector).await.unwrap();
    let port = forwarder.local_port();

    forwarder.shutdown().await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let err = TcpStream::connect(("127.0.0.1", port)).await.unwrap_err();
    assert!(
        matches!(
            err.kind(),
            std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::TimedOut
        ),
        "unexpected error kind: {:?}",
        err.kind()
    );
}
