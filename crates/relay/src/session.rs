use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Instant,
};

use mobilecode_connect_auth::TokenError;
use mobilecode_connect_protocol::{RelayLimits, SessionId, TrafficStats};
use mobilecode_connect_tunnel::{quic::QuicError, stream::TunnelStreamError};

use crate::bind::{
    RelayBindRequest, RelayBindStatus, RelayPeer, RelayPeerRole, RelayTokenVerifier,
};
use crate::forward::RelayForwardError;
use crate::limiter::RelayLimiter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelaySession {
    pub session_id: SessionId,
    pub mobile: Option<RelayPeer>,
    pub agent: Option<RelayPeer>,
    pub limits: RelayLimits,
    pub stats: TrafficStats,
    pub started_at: Instant,
    pub state: RelaySessionState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelaySessionState {
    Waiting,
    Ready,
    Closed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelaySessionMetrics {
    pub active_sessions: u64,
    pub active_streams: u64,
    pub total_uplink_bytes: u64,
    pub total_downlink_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone)]
pub struct RelaySessionStore {
    sessions: Arc<RwLock<HashMap<SessionId, RelaySession>>>,
    limiters: Arc<RwLock<HashMap<SessionId, RelayLimiter>>>,
    verifier: Arc<dyn RelayTokenVerifier>,
}

impl RelaySessionStore {
    pub fn new(verifier: Arc<dyn RelayTokenVerifier>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            limiters: Arc::new(RwLock::new(HashMap::new())),
            verifier,
        }
    }

    pub fn bind(&self, request: RelayBindRequest) -> Result<RelayBindStatus, RelayError> {
        let claims = self.verifier.verify(&request.token)?;
        if claims.session_id != request.session_id {
            return Err(RelayError::SessionMismatch {
                requested: request.session_id,
                token: claims.session_id,
            });
        }

        let mut sessions = self.sessions.write().expect("relay session lock poisoned");
        let session = sessions
            .entry(claims.session_id.clone())
            .or_insert_with(|| RelaySession {
                session_id: claims.session_id.clone(),
                mobile: None,
                agent: None,
                limits: RelayLimits {
                    max_bps: claims.max_bps,
                    max_streams: claims.max_streams,
                    max_duration_sec: claims.max_duration_sec,
                    traffic_quota_bytes: claims.traffic_quota_bytes,
                },
                stats: TrafficStats {
                    session_id: Some(claims.session_id.clone()),
                    ..TrafficStats::default()
                },
                started_at: Instant::now(),
                state: RelaySessionState::Waiting,
            });
        self.limiters
            .write()
            .expect("relay limiter lock poisoned")
            .entry(claims.session_id.clone())
            .or_insert_with(|| RelayLimiter::new(claims.max_bps));

        if session.state == RelaySessionState::Closed {
            return Err(RelayError::SessionClosed {
                session_id: session.session_id.clone(),
            });
        }

        match request.role {
            RelayPeerRole::Mobile => {
                if session.mobile.is_some() {
                    return Err(RelayError::DuplicateRole {
                        session_id: session.session_id.clone(),
                        role: RelayPeerRole::Mobile,
                    });
                }
                session.mobile = Some(RelayPeer::new(RelayPeerRole::Mobile));
            }
            RelayPeerRole::Agent => {
                if session.agent.is_some() {
                    return Err(RelayError::DuplicateRole {
                        session_id: session.session_id.clone(),
                        role: RelayPeerRole::Agent,
                    });
                }
                session.agent = Some(RelayPeer::new(RelayPeerRole::Agent));
            }
        }

        if session.mobile.is_some() && session.agent.is_some() {
            session.state = RelaySessionState::Ready;
            Ok(RelayBindStatus::Ready)
        } else {
            session.state = RelaySessionState::Waiting;
            Ok(RelayBindStatus::Waiting)
        }
    }

    pub fn get(&self, session_id: &SessionId) -> Option<RelaySession> {
        self.sessions
            .read()
            .expect("relay session lock poisoned")
            .get(session_id)
            .map(session_with_duration)
    }

    pub fn list(&self) -> Vec<RelaySession> {
        let mut sessions: Vec<_> = self
            .sessions
            .read()
            .expect("relay session lock poisoned")
            .values()
            .map(session_with_duration)
            .collect();
        sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        sessions
    }

    pub fn metrics(&self) -> RelaySessionMetrics {
        let mut metrics = RelaySessionMetrics::default();
        for session in self.list() {
            if session.state != RelaySessionState::Closed {
                metrics.active_sessions = metrics.active_sessions.saturating_add(1);
            }
            metrics.active_streams = metrics
                .active_streams
                .saturating_add(u64::from(session.stats.active_streams));
            metrics.total_uplink_bytes = metrics
                .total_uplink_bytes
                .saturating_add(session.stats.uplink_bytes);
            metrics.total_downlink_bytes = metrics
                .total_downlink_bytes
                .saturating_add(session.stats.downlink_bytes);
            metrics.total_bytes = metrics
                .total_bytes
                .saturating_add(session.stats.total_bytes);
        }
        metrics
    }

    pub fn begin_stream(&self, session_id: &SessionId) -> Result<RelayStreamPermit, RelayError> {
        let mut sessions = self.sessions.write().expect("relay session lock poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| RelayError::SessionNotFound {
                session_id: session_id.clone(),
            })?;

        refresh_duration(session);
        if session.state == RelaySessionState::Closed {
            return Err(RelayError::SessionClosed {
                session_id: session_id.clone(),
            });
        }
        if session.stats.duration_sec >= session.limits.max_duration_sec {
            session.state = RelaySessionState::Closed;
            return Err(RelayError::SessionExpired {
                session_id: session_id.clone(),
                max_duration_sec: session.limits.max_duration_sec,
            });
        }
        if quota_reached(session) {
            session.state = RelaySessionState::Closed;
            return Err(RelayError::TrafficQuotaExceeded {
                session_id: session_id.clone(),
                quota_bytes: session.limits.traffic_quota_bytes,
                total_bytes: session.stats.total_bytes,
            });
        }
        if session.stats.active_streams >= session.limits.max_streams {
            return Err(RelayError::MaxStreamsExceeded {
                session_id: session_id.clone(),
                max_streams: session.limits.max_streams,
            });
        }

        session.stats.active_streams += 1;
        let limiter = self
            .limiters
            .read()
            .expect("relay limiter lock poisoned")
            .get(session_id)
            .cloned()
            .unwrap_or_else(|| RelayLimiter::new(session.limits.max_bps));
        Ok(RelayStreamPermit {
            store: self.clone(),
            session_id: session_id.clone(),
            max_bps: session.limits.max_bps,
            limiter,
            active: true,
        })
    }

    pub fn add_traffic(
        &self,
        session_id: &SessionId,
        uplink_bytes: u64,
        downlink_bytes: u64,
    ) -> Result<(), RelayError> {
        let mut sessions = self.sessions.write().expect("relay session lock poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| RelayError::SessionNotFound {
                session_id: session_id.clone(),
            })?;
        refresh_duration(session);
        add_traffic_to_session(session, uplink_bytes, downlink_bytes);
        close_if_quota_exceeded(session)
    }

    pub fn close(&self, session_id: &SessionId) -> Result<RelaySession, RelayError> {
        let mut sessions = self.sessions.write().expect("relay session lock poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| RelayError::SessionNotFound {
                session_id: session_id.clone(),
            })?;
        refresh_duration(session);
        session.state = RelaySessionState::Closed;
        session.stats.active_streams = 0;
        Ok(session.clone())
    }

    fn finish_stream(
        &self,
        session_id: &SessionId,
        uplink_bytes: u64,
        downlink_bytes: u64,
    ) -> Result<(), RelayError> {
        let mut sessions = self.sessions.write().expect("relay session lock poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| RelayError::SessionNotFound {
                session_id: session_id.clone(),
            })?;
        refresh_duration(session);
        end_stream(session);
        add_traffic_to_session(session, uplink_bytes, downlink_bytes);
        close_if_quota_exceeded(session)
    }

    fn end_stream(&self, session_id: &SessionId) {
        if let Some(session) = self
            .sessions
            .write()
            .expect("relay session lock poisoned")
            .get_mut(session_id)
        {
            end_stream(session);
        }
    }
}

pub struct RelayStreamPermit {
    store: RelaySessionStore,
    session_id: SessionId,
    max_bps: u64,
    limiter: RelayLimiter,
    active: bool,
}

impl RelayStreamPermit {
    pub fn max_bps(&self) -> u64 {
        self.max_bps
    }

    pub fn limiter(&self) -> RelayLimiter {
        self.limiter.clone()
    }

    pub fn finish(mut self, uplink_bytes: u64, downlink_bytes: u64) -> Result<(), RelayError> {
        let result = self
            .store
            .finish_stream(&self.session_id, uplink_bytes, downlink_bytes);
        self.active = false;
        result
    }
}

impl std::fmt::Debug for RelayStreamPermit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RelayStreamPermit")
            .field("session_id", &self.session_id)
            .field("max_bps", &self.max_bps)
            .field("active", &self.active)
            .finish()
    }
}

impl Drop for RelayStreamPermit {
    fn drop(&mut self) {
        if self.active {
            self.store.end_stream(&self.session_id);
        }
    }
}

fn session_with_duration(session: &RelaySession) -> RelaySession {
    let mut session = session.clone();
    refresh_duration(&mut session);
    session
}

fn refresh_duration(session: &mut RelaySession) {
    session.stats.duration_sec = session.started_at.elapsed().as_secs();
}

fn end_stream(session: &mut RelaySession) {
    session.stats.active_streams = session.stats.active_streams.saturating_sub(1);
}

fn add_traffic_to_session(session: &mut RelaySession, uplink_bytes: u64, downlink_bytes: u64) {
    session.stats.uplink_bytes = session.stats.uplink_bytes.saturating_add(uplink_bytes);
    session.stats.downlink_bytes = session.stats.downlink_bytes.saturating_add(downlink_bytes);
    session.stats.total_bytes = session
        .stats
        .uplink_bytes
        .saturating_add(session.stats.downlink_bytes);
}

fn quota_reached(session: &RelaySession) -> bool {
    session.limits.traffic_quota_bytes > 0
        && session.stats.total_bytes >= session.limits.traffic_quota_bytes
}

fn close_if_quota_exceeded(session: &mut RelaySession) -> Result<(), RelayError> {
    if session.limits.traffic_quota_bytes > 0
        && session.stats.total_bytes > session.limits.traffic_quota_bytes
    {
        session.state = RelaySessionState::Closed;
        return Err(RelayError::TrafficQuotaExceeded {
            session_id: session.session_id.clone(),
            quota_bytes: session.limits.traffic_quota_bytes,
            total_bytes: session.stats.total_bytes,
        });
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error("relay token invalid: {source}")]
    TokenInvalid { source: TokenError },
    #[error("session mismatch: requested {requested}, token {token}")]
    SessionMismatch {
        requested: SessionId,
        token: SessionId,
    },
    #[error("duplicate {role:?} bind for session {session_id}")]
    DuplicateRole {
        session_id: SessionId,
        role: RelayPeerRole,
    },
    #[error("quic endpoint failed: {0}")]
    Quic(#[from] QuicError),
    #[error("quic connection failed: {0}")]
    Connection(#[from] quinn::ConnectionError),
    #[error("stream failed: {0}")]
    Stream(#[from] TunnelStreamError),
    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("relay forward failed: {0}")]
    Forward(#[from] RelayForwardError),
    #[error("unexpected control frame during relay bind")]
    UnexpectedControlFrame,
    #[error("agent connection missing for session {session_id}")]
    AgentConnectionMissing { session_id: SessionId },
    #[error("session not found: {session_id}")]
    SessionNotFound { session_id: SessionId },
    #[error("session closed: {session_id}")]
    SessionClosed { session_id: SessionId },
    #[error("session expired: {session_id}, max duration {max_duration_sec}s")]
    SessionExpired {
        session_id: SessionId,
        max_duration_sec: u64,
    },
    #[error("max streams exceeded for session {session_id}: {max_streams}")]
    MaxStreamsExceeded {
        session_id: SessionId,
        max_streams: u32,
    },
    #[error("traffic quota exceeded for session {session_id}: {total_bytes}/{quota_bytes}")]
    TrafficQuotaExceeded {
        session_id: SessionId,
        quota_bytes: u64,
        total_bytes: u64,
    },
}
