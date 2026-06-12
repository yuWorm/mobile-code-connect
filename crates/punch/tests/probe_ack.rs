use std::{net::SocketAddr, time::Duration};

use mobilecode_connect_protocol::SessionId;
use mobilecode_connect_punch::probe::{
    run_probe, PunchAck, PunchPacket, PunchProbe, PunchProbeConfig,
};
use tokio::net::UdpSocket;

#[test]
fn signed_probe_and_ack_verify_shared_secret() {
    let session_id = SessionId::new("sess_001");
    let probe = PunchProbe::signed(
        session_id.clone(),
        "mobile_001".to_string(),
        "pc_001".to_string(),
        "nonce_001".to_string(),
        1_767_000_000,
        "secret",
    )
    .unwrap();

    assert!(probe.verify("secret").is_ok());
    assert!(probe.verify("wrong-secret").is_err());

    let ack = PunchAck::signed(
        session_id,
        "pc_001".to_string(),
        "mobile_001".to_string(),
        probe.nonce.clone(),
        1_767_000_001,
        "secret",
    )
    .unwrap();

    assert!(ack.verify("secret").is_ok());
    assert!(ack.verify("wrong-secret").is_err());
    assert!(matches!(PunchPacket::Probe(probe), PunchPacket::Probe(_)));
    assert!(matches!(PunchPacket::Ack(ack), PunchPacket::Ack(_)));
}

#[tokio::test]
async fn local_peers_exchange_probe_ack_and_keep_the_successful_udp_socket() {
    let mobile_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let agent_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let mobile_addr = mobile_socket.local_addr().unwrap();
    let agent_addr = agent_socket.local_addr().unwrap();
    let session_id = SessionId::new("sess_001");

    let mobile = run_probe(
        mobile_socket,
        config(session_id.clone(), "mobile_001", "pc_001", agent_addr),
    );
    let agent = run_probe(
        agent_socket,
        config(session_id, "pc_001", "mobile_001", mobile_addr),
    );

    let (mobile, agent) = tokio::try_join!(mobile, agent).unwrap();

    assert_eq!(mobile.peer_addr, agent_addr);
    assert_eq!(mobile.socket.local_addr().unwrap(), mobile_addr);
    assert_eq!(agent.peer_addr, mobile_addr);
    assert_eq!(agent.socket.local_addr().unwrap(), agent_addr);
}

fn config(
    session_id: SessionId,
    self_id: &str,
    peer_id: &str,
    peer_addr: SocketAddr,
) -> PunchProbeConfig {
    PunchProbeConfig {
        session_id,
        self_id: self_id.to_string(),
        peer_id: peer_id.to_string(),
        peer_addr,
        shared_secret: "secret".to_string(),
        timeout: Duration::from_secs(1),
        probe_interval: Duration::from_millis(10),
    }
}
