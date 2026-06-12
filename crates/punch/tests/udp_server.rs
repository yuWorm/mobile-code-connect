use std::time::Duration;

use quic_tunnel_protocol::{CandidateSource, CandidateType, PeerRole, SessionId};
use quic_tunnel_punch::{
    probe::{PunchHello, PunchPacket},
    server::{PunchServer, PUNCH_BUFFER_SIZE},
};
use tokio::{net::UdpSocket, sync::oneshot};

#[tokio::test]
async fn udp_hello_records_public_candidate_and_returns_session_candidates() {
    let server = PunchServer::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let server_addr = server.local_addr().unwrap();
    let store = server.store();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(server.run_until(async {
        let _ = shutdown_rx.await;
    }));

    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let client_addr = client.local_addr().unwrap();
    let session_id = SessionId::new("sess_001");
    let packet = PunchPacket::Hello(PunchHello {
        session_id: session_id.clone(),
        role: PeerRole::Mobile,
        peer_id: "mobile_001".to_string(),
    });
    client
        .send_to(&packet.encode().unwrap(), server_addr)
        .await
        .unwrap();

    let mut buf = [0_u8; PUNCH_BUFFER_SIZE];
    let (len, _) = tokio::time::timeout(Duration::from_secs(1), client.recv_from(&mut buf))
        .await
        .unwrap()
        .unwrap();
    let response = PunchPacket::decode(&buf[..len]).unwrap();

    let PunchPacket::Candidates {
        session_id: returned_session_id,
        candidates,
    } = response
    else {
        panic!("expected candidates response");
    };
    assert_eq!(returned_session_id, session_id);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].role, PeerRole::Mobile);
    assert_eq!(candidates[0].peer_id, "mobile_001");
    assert_eq!(candidates[0].candidate_type, CandidateType::Srflx);
    assert_eq!(candidates[0].addr, client_addr.to_string());
    assert_eq!(candidates[0].source, CandidateSource::PunchServer);
    assert_eq!(store.list(&session_id), candidates);

    let _ = shutdown_tx.send(());
    server_task.await.unwrap().unwrap();
}
