use mobilecode_connect_protocol::{
    ControlFrame, DataStreamHeader, PeerRole, RelayBindFrame, ServiceId, SessionId, StreamId,
};
use mobilecode_connect_tunnel::stream::{
    read_control_frame, read_data_header, write_control_frame, write_data_header,
};
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

fn test_header() -> DataStreamHeader {
    DataStreamHeader {
        stream_id: StreamId::new("stream_001"),
        session_id: SessionId::new("sess_001"),
        service_id: ServiceId::new("svc_web_3000"),
    }
}

#[tokio::test]
async fn writes_and_reads_data_header_without_consuming_payload() {
    let (mut client, mut server) = duplex(1024);
    let expected = test_header();

    let writer = tokio::spawn(async move {
        write_data_header(&mut client, &expected).await.unwrap();
        client.write_all(b"hello").await.unwrap();
    });

    let actual = read_data_header(&mut server).await.unwrap();
    let mut payload = [0_u8; 5];
    server.read_exact(&mut payload).await.unwrap();

    writer.await.unwrap();
    assert_eq!(actual, test_header());
    assert_eq!(&payload, b"hello");
}

#[tokio::test]
async fn read_data_header_rejects_oversized_header_before_allocation() {
    let (mut client, mut server) = duplex(16);
    client.write_all(&(65_537_u32).to_be_bytes()).await.unwrap();
    drop(client);

    let err = read_data_header(&mut server).await.unwrap_err();

    assert!(err.to_string().contains("too large"));
}

#[tokio::test]
async fn writes_and_reads_control_frame() {
    let (mut client, mut server) = duplex(1024);
    let expected = ControlFrame::RelayBind(RelayBindFrame {
        role: PeerRole::Mobile,
        session_id: SessionId::new("sess_001"),
        token: "relay-token".to_string(),
    });

    write_control_frame(&mut client, &expected).await.unwrap();
    let actual = read_control_frame(&mut server).await.unwrap();

    assert_eq!(actual, expected);
}
