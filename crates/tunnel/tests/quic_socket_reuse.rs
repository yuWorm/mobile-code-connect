use quic_tunnel_tunnel::quic::{
    generate_self_signed_server_identity, make_client_endpoint,
    make_client_endpoint_from_std_socket, make_server_endpoint,
    make_server_endpoint_from_std_socket, make_server_endpoint_from_std_socket_with_identity,
    P2pQuicIdentity, QuicBiStream,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UdpSocket,
};

#[tokio::test]
async fn client_quic_endpoint_reuses_successful_probe_udp_socket() {
    let server = make_server_endpoint("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let cert = server.certificate_der().clone();
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let probe_addr = socket.local_addr().unwrap();
    let socket = socket.into_std().unwrap();

    let endpoint = make_client_endpoint_from_std_socket(socket, &[cert])
        .await
        .unwrap();

    assert_eq!(endpoint.local_addr().unwrap(), probe_addr);
}

#[tokio::test]
async fn server_quic_endpoint_reuses_successful_probe_udp_socket() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let probe_addr = socket.local_addr().unwrap();

    let server = make_server_endpoint_from_std_socket(socket.into_std().unwrap())
        .await
        .unwrap();
    let server_cert = server.certificate_der().clone();
    assert_eq!(server.local_addr().unwrap(), probe_addr);

    let server_task = tokio::spawn(async move {
        let incoming = server.endpoint().accept().await.unwrap();
        let connection = incoming.await.unwrap();
        let (send, recv) = connection.accept_bi().await.unwrap();
        let mut stream = QuicBiStream::new(send, recv);
        let mut payload = [0_u8; 4];
        stream.read_exact(&mut payload).await.unwrap();
        stream.write_all(&payload).await.unwrap();
        stream.flush().await.unwrap();
        stream.shutdown().await.unwrap();
        let _ = connection.closed().await;
    });

    let client = make_client_endpoint("127.0.0.1:0".parse().unwrap(), &[server_cert])
        .await
        .unwrap();
    let connection = client
        .connect(probe_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    let (send, recv) = connection.open_bi().await.unwrap();
    let mut stream = QuicBiStream::new(send, recv);

    stream.write_all(b"ping").await.unwrap();
    stream.flush().await.unwrap();
    let mut payload = [0_u8; 4];
    stream.read_exact(&mut payload).await.unwrap();

    assert_eq!(&payload, b"ping");
    connection.close(0_u32.into(), b"done");
    client.wait_idle().await;
    server_task.await.unwrap();
}

#[tokio::test]
async fn server_quic_identity_roundtrips_from_der_parts() {
    let identity = generate_self_signed_server_identity().unwrap();
    let restored = P2pQuicIdentity::from_der_parts(
        identity.certificate_der().as_ref().to_vec(),
        identity.private_key_der().to_vec(),
    );
    let server_cert = restored.certificate_der().clone();
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = socket.local_addr().unwrap();

    let server =
        make_server_endpoint_from_std_socket_with_identity(socket.into_std().unwrap(), restored)
            .await
            .unwrap();
    assert_eq!(server.certificate_der(), &server_cert);

    let server_task = tokio::spawn(async move {
        let incoming = server.endpoint().accept().await.unwrap();
        let connection = incoming.await.unwrap();
        let (send, recv) = connection.accept_bi().await.unwrap();
        let mut stream = QuicBiStream::new(send, recv);
        let mut payload = [0_u8; 4];
        stream.read_exact(&mut payload).await.unwrap();
        stream.write_all(b"pong").await.unwrap();
        stream.flush().await.unwrap();
        stream.shutdown().await.unwrap();
        let _ = connection.closed().await;
    });

    let client = make_client_endpoint("127.0.0.1:0".parse().unwrap(), &[server_cert])
        .await
        .unwrap();
    let connection = client
        .connect(server_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    let (send, recv) = connection.open_bi().await.unwrap();
    let mut stream = QuicBiStream::new(send, recv);
    stream.write_all(b"ping").await.unwrap();
    stream.flush().await.unwrap();
    let mut payload = [0_u8; 4];
    stream.read_exact(&mut payload).await.unwrap();

    assert_eq!(&payload, b"pong");
    connection.close(0_u32.into(), b"done");
    client.wait_idle().await;
    server_task.await.unwrap();
}

#[tokio::test]
async fn server_quic_endpoint_reuses_punch_socket_with_configured_identity() {
    let identity = generate_self_signed_server_identity().unwrap();
    let server_cert = identity.certificate_der().clone();
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let probe_addr = socket.local_addr().unwrap();

    let server =
        make_server_endpoint_from_std_socket_with_identity(socket.into_std().unwrap(), identity)
            .await
            .unwrap();
    assert_eq!(server.certificate_der(), &server_cert);
    assert_eq!(server.local_addr().unwrap(), probe_addr);

    let server_task = tokio::spawn(async move {
        let incoming = server.endpoint().accept().await.unwrap();
        let connection = incoming.await.unwrap();
        let (send, recv) = connection.accept_bi().await.unwrap();
        let mut stream = QuicBiStream::new(send, recv);
        let mut payload = [0_u8; 4];
        stream.read_exact(&mut payload).await.unwrap();
        stream.write_all(b"pong").await.unwrap();
        stream.flush().await.unwrap();
        stream.shutdown().await.unwrap();
        let _ = connection.closed().await;
    });

    let client = make_client_endpoint("127.0.0.1:0".parse().unwrap(), &[server_cert])
        .await
        .unwrap();
    let connection = client
        .connect(probe_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    let (send, recv) = connection.open_bi().await.unwrap();
    let mut stream = QuicBiStream::new(send, recv);
    stream.write_all(b"ping").await.unwrap();
    stream.flush().await.unwrap();
    let mut payload = [0_u8; 4];
    stream.read_exact(&mut payload).await.unwrap();

    assert_eq!(&payload, b"pong");
    connection.close(0_u32.into(), b"done");
    client.wait_idle().await;
    server_task.await.unwrap();
}
