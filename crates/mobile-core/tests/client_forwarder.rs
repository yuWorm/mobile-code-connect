use std::sync::Arc;

use mobilecode_connect_control_client::HttpControlClientOptions;
use mobilecode_connect_mobile_core::{
    client::{OpenServiceRequest, TunnelClient},
    config::TunnelConfig,
    forward::MemoryStreamConnector,
};
use mobilecode_connect_protocol::{ClientId, DeviceId, ServiceId};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

fn config() -> TunnelConfig {
    TunnelConfig {
        user_token: "user-token".to_string(),
        control_server_url: "http://127.0.0.1:4242".to_string(),
        client_id: ClientId::new("mobile_001"),
        control_client_options: HttpControlClientOptions::default(),
    }
}

#[tokio::test]
async fn tunnel_client_open_service_starts_local_forwarder() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let client = TunnelClient::with_connector(config(), connector.clone())
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
    let mut remote = connector.accept().await.unwrap();

    local.write_all(b"ping").await.unwrap();
    let mut remote_payload = [0_u8; 4];
    remote.read_exact(&mut remote_payload).await.unwrap();
    remote.write_all(b"pong").await.unwrap();

    let mut local_payload = [0_u8; 4];
    local.read_exact(&mut local_payload).await.unwrap();

    assert_eq!(&remote_payload, b"ping");
    assert_eq!(&local_payload, b"pong");

    client
        .close_service(handle.handle_id().to_string())
        .await
        .unwrap();
}
