use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use mobilecode_connect_protocol::TrafficStats;

#[derive(Debug, Default)]
pub struct AtomicTrafficStats {
    uplink_bytes: AtomicU64,
    downlink_bytes: AtomicU64,
    active_streams: AtomicU32,
}

impl AtomicTrafficStats {
    pub fn add_uplink(&self, bytes: u64) {
        self.uplink_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_downlink(&self, bytes: u64) {
        self.downlink_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn begin_stream(&self) {
        self.active_streams.fetch_add(1, Ordering::Relaxed);
    }

    pub fn end_stream(&self) {
        let _ = self
            .active_streams
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_sub(1)
            });
    }

    pub fn snapshot(&self) -> TrafficStats {
        let uplink_bytes = self.uplink_bytes.load(Ordering::Relaxed);
        let downlink_bytes = self.downlink_bytes.load(Ordering::Relaxed);

        TrafficStats {
            session_id: None,
            uplink_bytes,
            downlink_bytes,
            total_bytes: uplink_bytes + downlink_bytes,
            duration_sec: 0,
            active_streams: self.active_streams.load(Ordering::Relaxed),
        }
    }
}
