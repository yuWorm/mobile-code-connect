use mobilecode_connect_protocol::{DataStreamHeader, ServiceId, SessionId, StreamId};
use mobilecode_connect_relay::{forward::forward_stream_pair_with_limit, limiter::RelayLimiter};
use mobilecode_connect_tunnel::stream::{read_data_header, write_data_header};
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

fn header() -> DataStreamHeader {
    DataStreamHeader {
        stream_id: StreamId::new("stream_001"),
        session_id: SessionId::new("sess_001"),
        service_id: ServiceId::new("svc_web_3000"),
    }
}

#[tokio::test]
async fn forward_stream_pair_propagates_header_and_copies_both_directions() {
    let (mut mobile_peer, mobile_stream) = duplex(1024);
    let (agent_stream, mut agent_peer) = duplex(1024);

    let forward_task = tokio::spawn(async move {
        forward_stream_pair_with_limit(mobile_stream, agent_stream, RelayLimiter::new(0))
            .await
            .unwrap()
    });

    write_data_header(&mut mobile_peer, &header())
        .await
        .unwrap();
    mobile_peer.write_all(b"hello").await.unwrap();

    let forwarded_header = read_data_header(&mut agent_peer).await.unwrap();
    let mut forwarded_payload = [0_u8; 5];
    agent_peer.read_exact(&mut forwarded_payload).await.unwrap();
    agent_peer.write_all(b"world").await.unwrap();

    let mut response = [0_u8; 5];
    mobile_peer.read_exact(&mut response).await.unwrap();

    mobile_peer.shutdown().await.unwrap();
    agent_peer.shutdown().await.unwrap();

    let outcome = forward_task.await.unwrap();
    assert_eq!(forwarded_header, header());
    assert_eq!(&forwarded_payload, b"hello");
    assert_eq!(&response, b"world");
    assert_eq!(outcome.uplink_bytes, 5);
    assert_eq!(outcome.downlink_bytes, 5);
}
