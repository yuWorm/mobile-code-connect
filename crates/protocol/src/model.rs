use serde::{Deserialize, Serialize};

use crate::ids::{ClientId, DeviceId, ServiceId, SessionId, UserId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub device_id: DeviceId,
    pub user_id: UserId,
    pub name: String,
    pub status: DeviceStatus,
    pub agent_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Online,
    Offline,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    pub service_id: ServiceId,
    pub device_id: DeviceId,
    pub name: String,
    pub protocol: ServiceProtocol,
    pub target_host: String,
    pub target_port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceProtocol {
    Tcp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub client_id: ClientId,
    pub device_id: DeviceId,
    pub service_id: ServiceId,
    pub mode: SessionMode,
    pub expire_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    P2p,
    Relay,
    P2pOrRelay,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub candidate_type: CandidateType,
    pub addr: String,
    pub priority: u32,
    pub source: CandidateSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateType {
    Host,
    Srflx,
    Relay,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateSource {
    Local,
    PunchServer,
    Relay,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayLimits {
    pub max_bps: u64,
    pub max_streams: u32,
    pub max_duration_sec: u64,
    pub traffic_quota_bytes: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrafficStats {
    pub session_id: Option<SessionId>,
    pub uplink_bytes: u64,
    pub downlink_bytes: u64,
    pub total_bytes: u64,
    pub duration_sec: u64,
    pub active_streams: u32,
}
