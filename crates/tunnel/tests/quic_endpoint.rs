use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use quic_tunnel_tunnel::quic::{make_client_endpoint, make_server_endpoint, QuicBiStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn quic_client_and_server_exchange_bidirectional_stream() {
    let server = make_server_endpoint(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();
    let server_addr = server.local_addr().unwrap();
    let server_cert = server.certificate_der().clone();

    let server_task = tokio::spawn(async move {
        let incoming = server.endpoint().accept().await.unwrap();
        let connection = incoming.await.unwrap();
        let (send, recv) = connection.accept_bi().await.unwrap();
        let mut stream = QuicBiStream::new(send, recv);

        let mut payload = [0_u8; 4];
        stream.read_exact(&mut payload).await.unwrap();
        assert_eq!(&payload, b"ping");
        stream.write_all(b"pong").await.unwrap();
        stream.shutdown().await.unwrap();
        let _ = connection.closed().await;
    });

    let client = make_client_endpoint(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        &[server_cert],
    )
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
