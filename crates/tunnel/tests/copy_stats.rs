use std::sync::Arc;

use quic_tunnel_tunnel::{copy::copy_bidirectional_with_stats, stats::AtomicTrafficStats};
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn copy_bidirectional_updates_uplink_and_downlink_stats() {
    let (mut mobile_peer, mobile_stream) = duplex(64);
    let (agent_stream, mut agent_peer) = duplex(64);
    let stats = Arc::new(AtomicTrafficStats::default());
    let task_stats = Arc::clone(&stats);

    let copy_task = tokio::spawn(async move {
        copy_bidirectional_with_stats(mobile_stream, agent_stream, task_stats)
            .await
            .unwrap()
    });

    mobile_peer.write_all(b"up").await.unwrap();
    agent_peer.write_all(b"down").await.unwrap();

    let mut uplink_payload = [0_u8; 2];
    agent_peer.read_exact(&mut uplink_payload).await.unwrap();
    let mut downlink_payload = [0_u8; 4];
    mobile_peer.read_exact(&mut downlink_payload).await.unwrap();

    mobile_peer.shutdown().await.unwrap();
    agent_peer.shutdown().await.unwrap();

    let outcome = copy_task.await.unwrap();
    let snapshot = stats.snapshot();

    assert_eq!(&uplink_payload, b"up");
    assert_eq!(&downlink_payload, b"down");
    assert_eq!(outcome.uplink_bytes, 2);
    assert_eq!(outcome.downlink_bytes, 4);
    assert_eq!(snapshot.uplink_bytes, 2);
    assert_eq!(snapshot.downlink_bytes, 4);
    assert_eq!(snapshot.total_bytes, 6);
}

#[test]
fn atomic_traffic_stats_tracks_active_streams() {
    let stats = AtomicTrafficStats::default();

    stats.begin_stream();
    stats.begin_stream();
    stats.end_stream();

    assert_eq!(stats.snapshot().active_streams, 1);
}
