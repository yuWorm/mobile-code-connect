use std::{net::SocketAddr, str::FromStr};

use mobilecode_connect_agent::{
    config::ServiceConfig,
    service_registry::ServiceRegistry,
    stream_handler::{
        detected_lan_cidr_for_interface, handle_data_stream, handle_data_stream_with_policy,
        handle_data_stream_with_policy_and_resolver, AgentStreamError, TargetAccessPolicy,
    },
};
use mobilecode_connect_protocol::{
    DataStreamHeader, ServiceId, ServiceProtocol, SessionId, StreamId,
};
use mobilecode_connect_tunnel::stream::write_data_header;
use tokio::{
    io::{duplex, AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn service(id: &str, port: u16) -> ServiceConfig {
    ServiceConfig {
        service_id: ServiceId::new(id),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: port,
    }
}

fn header(service_id: &str) -> DataStreamHeader {
    DataStreamHeader {
        stream_id: StreamId::new("stream_001"),
        session_id: SessionId::new("sess_001"),
        service_id: ServiceId::new(service_id),
    }
}

#[tokio::test]
async fn handle_data_stream_proxies_bytes_to_local_service() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = listener.local_addr().unwrap().port();
    let echo_task = tokio::spawn(async move {
        let (mut tcp, _) = listener.accept().await.unwrap();
        let mut payload = [0_u8; 5];
        tcp.read_exact(&mut payload).await.unwrap();
        tcp.write_all(&payload).await.unwrap();
        tcp.shutdown().await.unwrap();
    });

    let registry = ServiceRegistry::new(vec![service("svc_web_3000", target_port)]).unwrap();
    let (mut client, server) = duplex(1024);
    let handler = tokio::spawn(handle_data_stream(server, registry));

    write_data_header(&mut client, &header("svc_web_3000"))
        .await
        .unwrap();
    client.write_all(b"hello").await.unwrap();
    client.shutdown().await.unwrap();

    let mut echoed = [0_u8; 5];
    client.read_exact(&mut echoed).await.unwrap();

    assert_eq!(&echoed, b"hello");
    handler.await.unwrap().unwrap();
    echo_task.await.unwrap();
}

#[tokio::test]
async fn handle_data_stream_rejects_unknown_service_id() {
    let registry = ServiceRegistry::new(vec![service("svc_web_3000", 3000)]).unwrap();
    let (mut client, server) = duplex(1024);
    let handler = tokio::spawn(handle_data_stream(server, registry));

    write_data_header(&mut client, &header("svc_missing"))
        .await
        .unwrap();
    drop(client);

    let err = handler.await.unwrap().unwrap_err();

    assert!(matches!(err, AgentStreamError::ServiceNotFound { .. }));
}

#[tokio::test]
async fn handle_data_stream_allows_target_inside_receiver_lan_policy() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = listener.local_addr().unwrap().port();
    let echo_task = tokio::spawn(async move {
        let (mut tcp, _) = listener.accept().await.unwrap();
        let mut payload = [0_u8; 4];
        tcp.read_exact(&mut payload).await.unwrap();
        tcp.write_all(&payload).await.unwrap();
    });

    let registry = ServiceRegistry::new(vec![service("svc_web_3000", target_port)]).unwrap();
    let policy = TargetAccessPolicy::with_allowed_lan_cidrs(["127.0.0.0/8"]).unwrap();
    let (mut client, server) = duplex(1024);
    let handler = tokio::spawn(handle_data_stream_with_policy(server, registry, policy));

    write_data_header(&mut client, &header("svc_web_3000"))
        .await
        .unwrap();
    client.write_all(b"ping").await.unwrap();
    client.shutdown().await.unwrap();

    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.unwrap();

    assert_eq!(&echoed, b"ping");
    handler.await.unwrap().unwrap();
    echo_task.await.unwrap();
}

#[tokio::test]
async fn handle_data_stream_blocks_target_outside_receiver_lan_policy() {
    let mut config = service("svc_web_3000", 3000);
    config.target_host = "192.168.1.50".to_string();
    let registry = ServiceRegistry::new(vec![config]).unwrap();
    let policy = TargetAccessPolicy::with_allowed_lan_cidrs(["10.0.0.0/8"]).unwrap();
    let (mut client, server) = duplex(1024);
    let handler = tokio::spawn(handle_data_stream_with_policy(server, registry, policy));

    write_data_header(&mut client, &header("svc_web_3000"))
        .await
        .unwrap();
    drop(client);

    let err = handler.await.unwrap().unwrap_err();

    assert!(matches!(err, AgentStreamError::TargetAccessDenied { .. }));
}

#[tokio::test]
async fn handle_data_stream_blocks_domain_that_resolves_outside_receiver_lan_policy() {
    let mut config = service("svc_web_3000", 3000);
    config.target_host = "public.example.test".to_string();
    let registry = ServiceRegistry::new(vec![config]).unwrap();
    let policy = TargetAccessPolicy::with_allowed_lan_cidrs(["10.0.0.0/8"]).unwrap();
    let (mut client, server) = duplex(1024);
    let handler = tokio::spawn(handle_data_stream_with_policy_and_resolver(
        server,
        registry,
        policy,
        |_host, port| async move {
            Ok(vec![
                SocketAddr::from_str(&format!("192.168.1.50:{port}")).unwrap()
            ])
        },
    ));

    write_data_header(&mut client, &header("svc_web_3000"))
        .await
        .unwrap();
    drop(client);

    let err = handler.await.unwrap().unwrap_err();

    assert!(matches!(err, AgentStreamError::TargetAccessDenied { .. }));
}

#[tokio::test]
async fn handle_data_stream_allows_domain_that_resolves_inside_receiver_lan_policy() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = listener.local_addr().unwrap().port();
    let echo_task = tokio::spawn(async move {
        let (mut tcp, _) = listener.accept().await.unwrap();
        let mut payload = [0_u8; 4];
        tcp.read_exact(&mut payload).await.unwrap();
        tcp.write_all(&payload).await.unwrap();
    });
    let mut config = service("svc_web_3000", target_port);
    config.target_host = "internal.example.test".to_string();
    let registry = ServiceRegistry::new(vec![config]).unwrap();
    let policy = TargetAccessPolicy::with_allowed_lan_cidrs(["127.0.0.0/8"]).unwrap();
    let (mut client, server) = duplex(1024);
    let handler = tokio::spawn(handle_data_stream_with_policy_and_resolver(
        server,
        registry,
        policy,
        move |_host, port| async move {
            Ok(vec![
                SocketAddr::from_str(&format!("127.0.0.1:{port}")).unwrap()
            ])
        },
    ));

    write_data_header(&mut client, &header("svc_web_3000"))
        .await
        .unwrap();
    client.write_all(b"pong").await.unwrap();
    client.shutdown().await.unwrap();

    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.unwrap();

    assert_eq!(&echoed, b"pong");
    handler.await.unwrap().unwrap();
    echo_task.await.unwrap();
}

#[test]
fn detected_lan_cidr_uses_interface_netmask() {
    assert_eq!(
        detected_lan_cidr_for_interface("192.168.1.42", "255.255.255.0").unwrap(),
        "192.168.1.0/24"
    );
    assert_eq!(
        detected_lan_cidr_for_interface("10.4.5.6", "255.0.0.0").unwrap(),
        "10.0.0.0/8"
    );
    assert!(detected_lan_cidr_for_interface("8.8.8.8", "255.255.255.0").is_none());
}
