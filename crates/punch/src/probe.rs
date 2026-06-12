use std::{
    net::SocketAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use mobilecode_connect_protocol::{PeerRole, SessionId};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tokio::net::UdpSocket;
use uuid::Uuid;

use crate::candidate::CandidateRecord;

type HmacSha256 = Hmac<Sha256>;
const PROBE_BUFFER_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PunchHello {
    pub session_id: SessionId,
    pub role: PeerRole,
    pub peer_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PunchProbe {
    pub session_id: SessionId,
    pub from: String,
    pub to: String,
    pub nonce: String,
    pub timestamp: u64,
    pub hmac: String,
}

impl PunchProbe {
    pub fn signed(
        session_id: SessionId,
        from: String,
        to: String,
        nonce: String,
        timestamp: u64,
        shared_secret: impl AsRef<[u8]>,
    ) -> Result<Self, PunchPacketError> {
        let hmac = sign_message(
            "PUNCH_PROBE",
            &session_id,
            &from,
            &to,
            &nonce,
            timestamp,
            shared_secret,
        )?;
        Ok(Self {
            session_id,
            from,
            to,
            nonce,
            timestamp,
            hmac,
        })
    }

    pub fn verify(&self, shared_secret: impl AsRef<[u8]>) -> Result<(), PunchPacketError> {
        verify_message(
            "PUNCH_PROBE",
            &self.session_id,
            &self.from,
            &self.to,
            &self.nonce,
            self.timestamp,
            &self.hmac,
            shared_secret,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PunchAck {
    pub session_id: SessionId,
    pub from: String,
    pub to: String,
    pub nonce: String,
    pub timestamp: u64,
    pub hmac: String,
}

impl PunchAck {
    pub fn signed(
        session_id: SessionId,
        from: String,
        to: String,
        nonce: String,
        timestamp: u64,
        shared_secret: impl AsRef<[u8]>,
    ) -> Result<Self, PunchPacketError> {
        let hmac = sign_message(
            "PUNCH_ACK",
            &session_id,
            &from,
            &to,
            &nonce,
            timestamp,
            shared_secret,
        )?;
        Ok(Self {
            session_id,
            from,
            to,
            nonce,
            timestamp,
            hmac,
        })
    }

    pub fn verify(&self, shared_secret: impl AsRef<[u8]>) -> Result<(), PunchPacketError> {
        verify_message(
            "PUNCH_ACK",
            &self.session_id,
            &self.from,
            &self.to,
            &self.nonce,
            self.timestamp,
            &self.hmac,
            shared_secret,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PunchPacket {
    Hello(PunchHello),
    Probe(PunchProbe),
    Ack(PunchAck),
    Candidates {
        session_id: SessionId,
        candidates: Vec<CandidateRecord>,
    },
    Error {
        message: String,
    },
}

impl PunchPacket {
    pub fn encode(&self) -> Result<Vec<u8>, PunchPacketError> {
        Ok(serde_json::to_vec(self)?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PunchPacketError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug, Clone)]
pub struct PunchProbeConfig {
    pub session_id: SessionId,
    pub self_id: String,
    pub peer_id: String,
    pub peer_addr: SocketAddr,
    pub shared_secret: String,
    pub timeout: Duration,
    pub probe_interval: Duration,
}

pub struct PunchProbeOutcome {
    pub peer_addr: SocketAddr,
    pub nonce: String,
    pub socket: UdpSocket,
}

#[derive(Debug, Clone)]
pub struct P2pPathConfig {
    pub session_id: SessionId,
    pub role: PeerRole,
    pub self_id: String,
    pub peer_id: String,
    pub bind_addr: SocketAddr,
    pub punch_addr: SocketAddr,
    pub shared_secret: String,
    pub candidate_timeout: Duration,
    pub probe_timeout: Duration,
    pub interval: Duration,
}

pub struct P2pPath {
    pub peer_id: String,
    pub peer_addr: SocketAddr,
    pub socket: UdpSocket,
}

pub async fn establish_p2p_path(config: P2pPathConfig) -> Result<P2pPath, P2pPathError> {
    let socket = UdpSocket::bind(config.bind_addr).await?;
    let peer_candidate = wait_for_peer_candidate(&socket, &config).await?;
    let peer_addr = parse_candidate_addr(&peer_candidate)?;
    let outcome = run_probe(
        socket,
        PunchProbeConfig {
            session_id: config.session_id,
            self_id: config.self_id,
            peer_id: config.peer_id.clone(),
            peer_addr,
            shared_secret: config.shared_secret,
            timeout: config.probe_timeout,
            probe_interval: config.interval,
        },
    )
    .await?;

    Ok(P2pPath {
        peer_id: config.peer_id,
        peer_addr: outcome.peer_addr,
        socket: outcome.socket,
    })
}

pub async fn run_probe(
    socket: UdpSocket,
    config: PunchProbeConfig,
) -> Result<PunchProbeOutcome, PunchProbeError> {
    let nonce = Uuid::new_v4().to_string();
    let deadline = tokio::time::Instant::now() + config.timeout;
    let mut interval = tokio::time::interval(config.probe_interval);
    let mut buf = [0_u8; PROBE_BUFFER_SIZE];
    let mut own_probe_acked_by: Option<SocketAddr> = None;
    let mut peer_probe_acked = false;

    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => {
                return Err(PunchProbeError::Timeout);
            }
            _ = interval.tick() => {
                let probe = PunchProbe::signed(
                    config.session_id.clone(),
                    config.self_id.clone(),
                    config.peer_id.clone(),
                    nonce.clone(),
                    current_epoch_sec(),
                    &config.shared_secret,
                )?;
                socket
                    .send_to(&PunchPacket::Probe(probe).encode()?, config.peer_addr)
                    .await?;
            }
            received = socket.recv_from(&mut buf) => {
                let (len, source_addr) = received?;
                match PunchPacket::decode(&buf[..len]) {
                    Ok(PunchPacket::Probe(probe)) => {
                        if accepts_probe(&probe, &config) {
                            probe.verify(&config.shared_secret)?;
                            let ack = PunchAck::signed(
                                config.session_id.clone(),
                                config.self_id.clone(),
                                config.peer_id.clone(),
                                probe.nonce,
                                current_epoch_sec(),
                                &config.shared_secret,
                            )?;
                            socket
                                .send_to(&PunchPacket::Ack(ack).encode()?, source_addr)
                                .await?;
                            peer_probe_acked = true;
                            if let Some(peer_addr) = own_probe_acked_by {
                                return Ok(PunchProbeOutcome {
                                    peer_addr,
                                    nonce,
                                    socket,
                                });
                            }
                        }
                    }
                    Ok(PunchPacket::Ack(ack)) => {
                        if accepts_ack(&ack, &config, &nonce) {
                            ack.verify(&config.shared_secret)?;
                            own_probe_acked_by = Some(source_addr);
                            if peer_probe_acked {
                                return Ok(PunchProbeOutcome {
                                    peer_addr: source_addr,
                                    nonce,
                                    socket,
                                });
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PunchPacketError {
    #[error("punch packet json failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("punch packet signature is invalid")]
    InvalidSignature,
}

#[derive(Debug, thiserror::Error)]
pub enum PunchProbeError {
    #[error("punch probe timed out")]
    Timeout,
    #[error("udp failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("packet failed: {0}")]
    Packet(#[from] PunchPacketError),
}

#[derive(Debug, thiserror::Error)]
pub enum P2pPathError {
    #[error("candidate discovery timed out")]
    CandidateTimeout,
    #[error("peer candidate addr is invalid: {value}")]
    InvalidCandidateAddr { value: String },
    #[error("udp failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("packet failed: {0}")]
    Packet(#[from] PunchPacketError),
    #[error("probe failed: {0}")]
    Probe(#[from] PunchProbeError),
}

async fn wait_for_peer_candidate(
    socket: &UdpSocket,
    config: &P2pPathConfig,
) -> Result<CandidateRecord, P2pPathError> {
    let deadline = tokio::time::Instant::now() + config.candidate_timeout;
    let mut interval = tokio::time::interval(config.interval);
    let mut buf = [0_u8; PROBE_BUFFER_SIZE];

    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => {
                return Err(P2pPathError::CandidateTimeout);
            }
            _ = interval.tick() => {
                let hello = PunchPacket::Hello(PunchHello {
                    session_id: config.session_id.clone(),
                    role: config.role.clone(),
                    peer_id: config.self_id.clone(),
                });
                socket.send_to(&hello.encode()?, config.punch_addr).await?;
            }
            received = socket.recv_from(&mut buf) => {
                let (len, source_addr) = received?;
                if source_addr != config.punch_addr {
                    continue;
                }
                if let PunchPacket::Candidates { session_id, candidates } =
                    PunchPacket::decode(&buf[..len])?
                {
                    if session_id != config.session_id {
                        continue;
                    }
                    if let Some(candidate) = candidates
                        .into_iter()
                        .find(|candidate| candidate.peer_id == config.peer_id)
                    {
                        return Ok(candidate);
                    }
                }
            }
        }
    }
}

fn parse_candidate_addr(candidate: &CandidateRecord) -> Result<SocketAddr, P2pPathError> {
    candidate
        .addr
        .parse::<SocketAddr>()
        .map_err(|_| P2pPathError::InvalidCandidateAddr {
            value: candidate.addr.clone(),
        })
}

fn accepts_probe(probe: &PunchProbe, config: &PunchProbeConfig) -> bool {
    probe.session_id == config.session_id
        && probe.from == config.peer_id
        && probe.to == config.self_id
}

fn accepts_ack(ack: &PunchAck, config: &PunchProbeConfig, nonce: &str) -> bool {
    ack.session_id == config.session_id
        && ack.from == config.peer_id
        && ack.to == config.self_id
        && ack.nonce == nonce
}

fn sign_message(
    message_type: &str,
    session_id: &SessionId,
    from: &str,
    to: &str,
    nonce: &str,
    timestamp: u64,
    shared_secret: impl AsRef<[u8]>,
) -> Result<String, PunchPacketError> {
    let mut mac = HmacSha256::new_from_slice(shared_secret.as_ref())
        .map_err(|_| PunchPacketError::InvalidSignature)?;
    mac.update(canonical_message(message_type, session_id, from, to, nonce, timestamp).as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

fn verify_message(
    message_type: &str,
    session_id: &SessionId,
    from: &str,
    to: &str,
    nonce: &str,
    timestamp: u64,
    hmac: &str,
    shared_secret: impl AsRef<[u8]>,
) -> Result<(), PunchPacketError> {
    let signature = URL_SAFE_NO_PAD
        .decode(hmac)
        .map_err(|_| PunchPacketError::InvalidSignature)?;
    let mut mac = HmacSha256::new_from_slice(shared_secret.as_ref())
        .map_err(|_| PunchPacketError::InvalidSignature)?;
    mac.update(canonical_message(message_type, session_id, from, to, nonce, timestamp).as_bytes());
    mac.verify_slice(&signature)
        .map_err(|_| PunchPacketError::InvalidSignature)
}

fn canonical_message(
    message_type: &str,
    session_id: &SessionId,
    from: &str,
    to: &str,
    nonce: &str,
    timestamp: u64,
) -> String {
    format!("{message_type}|{session_id}|{from}|{to}|{nonce}|{timestamp}")
}

fn current_epoch_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before unix epoch")
        .as_secs()
}
