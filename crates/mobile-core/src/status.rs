use std::sync::{
    atomic::{AtomicU64, AtomicU8, Ordering},
    Arc,
};

use crate::path::TunnelPath;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelStatus {
    pub state: TunnelState,
    pub path: TunnelPath,
    pub rtt_ms: Option<u64>,
    pub uplink_bytes: u64,
    pub downlink_bytes: u64,
    pub active_forwards: usize,
    pub transport: TunnelTransportStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelState {
    Started,
    Connected,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TunnelTransportStats {
    pub p2p_attempts: u64,
    pub p2p_connections: u64,
    pub p2p_failures: u64,
    pub relay_fallbacks: u64,
    pub relay_connections: u64,
    pub relay_failures: u64,
    pub last_successful_path: Option<TunnelPath>,
}

#[derive(Clone, Default)]
pub struct TunnelTransportStatsHandle {
    counters: Arc<TunnelTransportStatsCounters>,
}

#[derive(Default)]
struct TunnelTransportStatsCounters {
    p2p_attempts: AtomicU64,
    p2p_connections: AtomicU64,
    p2p_failures: AtomicU64,
    relay_fallbacks: AtomicU64,
    relay_connections: AtomicU64,
    relay_failures: AtomicU64,
    last_successful_path: AtomicU8,
}

impl TunnelTransportStatsHandle {
    pub fn snapshot(&self) -> TunnelTransportStats {
        TunnelTransportStats {
            p2p_attempts: self.load(&self.counters.p2p_attempts),
            p2p_connections: self.load(&self.counters.p2p_connections),
            p2p_failures: self.load(&self.counters.p2p_failures),
            relay_fallbacks: self.load(&self.counters.relay_fallbacks),
            relay_connections: self.load(&self.counters.relay_connections),
            relay_failures: self.load(&self.counters.relay_failures),
            last_successful_path: self.last_successful_path(),
        }
    }

    pub(crate) fn p2p_attempt(&self) {
        Self::increment(&self.counters.p2p_attempts);
    }

    pub(crate) fn p2p_connection(&self) {
        Self::increment(&self.counters.p2p_connections);
        self.store_last_successful_path(TunnelPath::P2p);
    }

    pub(crate) fn p2p_failure(&self) {
        Self::increment(&self.counters.p2p_failures);
    }

    pub(crate) fn relay_fallback(&self) {
        Self::increment(&self.counters.relay_fallbacks);
    }

    pub(crate) fn relay_connection(&self) {
        Self::increment(&self.counters.relay_connections);
        self.store_last_successful_path(TunnelPath::Relay);
    }

    pub(crate) fn relay_failure(&self) {
        Self::increment(&self.counters.relay_failures);
    }

    fn load(&self, counter: &AtomicU64) -> u64 {
        counter.load(Ordering::Relaxed)
    }

    fn increment(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    fn last_successful_path(&self) -> Option<TunnelPath> {
        match self.counters.last_successful_path.load(Ordering::Relaxed) {
            1 => Some(TunnelPath::Relay),
            2 => Some(TunnelPath::P2p),
            _ => None,
        }
    }

    fn store_last_successful_path(&self, path: TunnelPath) {
        let encoded = match path {
            TunnelPath::Relay => 1,
            TunnelPath::P2p => 2,
        };
        self.counters
            .last_successful_path
            .store(encoded, Ordering::Relaxed);
    }
}
