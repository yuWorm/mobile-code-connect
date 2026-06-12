use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct RelayLimiter {
    state: Option<Arc<Mutex<LimiterState>>>,
}

#[derive(Debug)]
struct LimiterState {
    bytes_per_second: u64,
    available_tokens: u128,
    last_refill: Instant,
}

impl RelayLimiter {
    pub fn new(max_bps: u64) -> Self {
        if max_bps == 0 {
            return Self { state: None };
        }

        Self {
            state: Some(Arc::new(Mutex::new(LimiterState {
                bytes_per_second: max_bps,
                available_tokens: max_bps as u128,
                last_refill: Instant::now(),
            }))),
        }
    }

    pub fn reserve_delay_at(&self, bytes: usize, now: Instant) -> Duration {
        let Some(state) = &self.state else {
            return Duration::ZERO;
        };
        if bytes == 0 {
            return Duration::ZERO;
        }

        let mut state = state.lock().expect("relay limiter lock poisoned");
        state.refill(now);

        let bytes = bytes as u128;
        if bytes <= state.available_tokens {
            state.available_tokens -= bytes;
            return Duration::ZERO;
        }

        let deficit = bytes - state.available_tokens;
        state.available_tokens = 0;
        let delay = transfer_duration(deficit, state.bytes_per_second);
        state.last_refill = now.checked_add(delay).unwrap_or(now);
        delay
    }

    pub async fn throttle(&self, bytes: usize) {
        let delay = self.reserve_delay_at(bytes, Instant::now());
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
    }
}

impl LimiterState {
    fn refill(&mut self, now: Instant) {
        if now <= self.last_refill {
            return;
        }

        let elapsed_nanos = now.duration_since(self.last_refill).as_nanos();
        let new_tokens =
            elapsed_nanos.saturating_mul(self.bytes_per_second as u128) / 1_000_000_000;
        if new_tokens == 0 {
            return;
        }

        let capacity = self.bytes_per_second as u128;
        self.available_tokens = (self.available_tokens + new_tokens).min(capacity);
        if self.available_tokens == capacity {
            self.last_refill = now;
            return;
        }

        let consumed_nanos =
            new_tokens.saturating_mul(1_000_000_000) / self.bytes_per_second as u128;
        if let Some(last_refill) = self.last_refill.checked_add(Duration::from_nanos(
            consumed_nanos.min(u64::MAX as u128) as u64,
        )) {
            self.last_refill = last_refill;
        }
    }
}

fn transfer_duration(bytes: u128, bytes_per_second: u64) -> Duration {
    let nanos = bytes
        .saturating_mul(1_000_000_000)
        .div_ceil(bytes_per_second as u128);
    Duration::from_nanos(nanos.min(u64::MAX as u128) as u64)
}
