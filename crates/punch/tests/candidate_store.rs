use std::net::SocketAddr;

use quic_tunnel_protocol::{CandidateSource, CandidateType, PeerRole, SessionId};
use quic_tunnel_punch::candidate::{CandidateRecord, CandidateStore};

fn addr(port: u16) -> SocketAddr {
    format!("203.0.113.10:{port}").parse().unwrap()
}

#[test]
fn candidate_store_records_public_addr_by_session_and_role() {
    let store = CandidateStore::default();
    let session_id = SessionId::new("sess_001");

    let record = store.record_observed(
        session_id.clone(),
        PeerRole::Mobile,
        "mobile_001".to_string(),
        addr(42000),
    );

    assert_eq!(
        record,
        CandidateRecord {
            session_id: session_id.clone(),
            role: PeerRole::Mobile,
            peer_id: "mobile_001".to_string(),
            candidate_type: CandidateType::Srflx,
            addr: "203.0.113.10:42000".to_string(),
            priority: 100,
            source: CandidateSource::PunchServer,
        }
    );
    assert_eq!(store.list(&session_id), vec![record]);
}

#[test]
fn candidate_store_keeps_one_record_per_session_and_role() {
    let store = CandidateStore::default();
    let session_id = SessionId::new("sess_001");

    store.record_observed(
        session_id.clone(),
        PeerRole::Mobile,
        "mobile_001".to_string(),
        addr(42000),
    );
    let latest = store.record_observed(
        session_id.clone(),
        PeerRole::Mobile,
        "mobile_001".to_string(),
        addr(42001),
    );

    assert_eq!(store.list(&session_id), vec![latest.clone()]);
    assert_eq!(latest.addr, "203.0.113.10:42001");
}

#[test]
fn candidate_store_lists_mobile_before_agent() {
    let store = CandidateStore::default();
    let session_id = SessionId::new("sess_001");
    store.record_observed(
        session_id.clone(),
        PeerRole::Agent,
        "pc_001".to_string(),
        addr(42002),
    );
    store.record_observed(
        session_id.clone(),
        PeerRole::Mobile,
        "mobile_001".to_string(),
        addr(42000),
    );

    let records = store.list(&session_id);

    assert_eq!(records[0].role, PeerRole::Mobile);
    assert_eq!(records[1].role, PeerRole::Agent);
}
