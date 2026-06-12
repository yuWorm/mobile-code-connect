use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use mobilecode_connect_protocol::{PeerRole, SessionId};
use mobilecode_connect_punch::{
    probe::{establish_p2p_path, P2pPathConfig},
    server::PunchServer,
};
use tokio::sync::oneshot;

#[tokio::test]
async fn peers_discover_candidates_through_punch_server_and_establish_p2p_path() {
    let server = PunchServer::bind(local_addr(0)).await.unwrap();
    let punch_addr = server.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(server.run_until(async {
        let _ = shutdown_rx.await;
    }));

    let session_id = SessionId::new("sess_001");
    let mobile = establish_p2p_path(config(
        session_id.clone(),
        PeerRole::Mobile,
        "mobile_001",
        "pc_001",
        punch_addr,
    ));
    let agent = establish_p2p_path(config(
        session_id,
        PeerRole::Agent,
        "pc_001",
        "mobile_001",
        punch_addr,
    ));

    let (mobile, agent) = tokio::try_join!(mobile, agent).unwrap();

    assert_eq!(mobile.peer_id, "pc_001");
    assert_eq!(agent.peer_id, "mobile_001");
    assert_eq!(mobile.peer_addr, agent.socket.local_addr().unwrap());
    assert_eq!(agent.peer_addr, mobile.socket.local_addr().unwrap());

    let _ = shutdown_tx.send(());
    server_task.await.unwrap().unwrap();
}

fn config(
    session_id: SessionId,
    role: PeerRole,
    self_id: &str,
    peer_id: &str,
    punch_addr: SocketAddr,
) -> P2pPathConfig {
    P2pPathConfig {
        session_id,
        role,
        self_id: self_id.to_string(),
        peer_id: peer_id.to_string(),
        bind_addr: local_addr(0),
        punch_addr,
        shared_secret: "secret".to_string(),
        candidate_timeout: Duration::from_secs(1),
        probe_timeout: Duration::from_secs(1),
        interval: Duration::from_millis(10),
    }
}

fn local_addr(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
}
