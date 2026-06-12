use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use quic_tunnel_protocol::{CandidateSource, CandidateType, PeerRole, SessionId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateRecord {
    pub session_id: SessionId,
    pub role: PeerRole,
    pub peer_id: String,
    pub candidate_type: CandidateType,
    pub addr: String,
    pub priority: u32,
    pub source: CandidateSource,
}

#[derive(Debug, Clone, Default)]
pub struct CandidateStore {
    records: Arc<RwLock<HashMap<CandidateKey, CandidateRecord>>>,
}

impl CandidateStore {
    pub fn record_observed(
        &self,
        session_id: SessionId,
        role: PeerRole,
        peer_id: String,
        public_addr: SocketAddr,
    ) -> CandidateRecord {
        let record = CandidateRecord {
            session_id: session_id.clone(),
            role: role.clone(),
            peer_id,
            candidate_type: CandidateType::Srflx,
            addr: public_addr.to_string(),
            priority: 100,
            source: CandidateSource::PunchServer,
        };

        self.records
            .write()
            .expect("candidate store lock poisoned")
            .insert(CandidateKey::new(session_id, &role), record.clone());
        record
    }

    pub fn list(&self, session_id: &SessionId) -> Vec<CandidateRecord> {
        let mut records: Vec<_> = self
            .records
            .read()
            .expect("candidate store lock poisoned")
            .values()
            .filter(|record| &record.session_id == session_id)
            .cloned()
            .collect();
        records.sort_by(|left, right| {
            role_rank(&left.role)
                .cmp(&role_rank(&right.role))
                .then_with(|| left.peer_id.cmp(&right.peer_id))
        });
        records
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CandidateKey {
    session_id: SessionId,
    role: CandidateRole,
}

impl CandidateKey {
    fn new(session_id: SessionId, role: &PeerRole) -> Self {
        Self {
            session_id,
            role: CandidateRole::from(role),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CandidateRole {
    Mobile,
    Agent,
}

impl From<&PeerRole> for CandidateRole {
    fn from(role: &PeerRole) -> Self {
        match role {
            PeerRole::Mobile => Self::Mobile,
            PeerRole::Agent => Self::Agent,
        }
    }
}

fn role_rank(role: &PeerRole) -> u8 {
    match role {
        PeerRole::Mobile => 0,
        PeerRole::Agent => 1,
    }
}
