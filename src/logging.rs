use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy)]
struct WarnThrottleState {
    last: Instant,
    suppressed: u64,
}

fn shorten(value: &str, head: usize, tail: usize) -> String {
    if value.len() <= head + tail {
        return value.to_string();
    }
    format!("{}..{}", &value[..head], &value[value.len() - tail..])
}

pub fn redact_hex(value: &str) -> String {
    shorten(value, 8, 8)
}

pub fn redact_b64(value: &str) -> String {
    shorten(value, 6, 6)
}

pub fn warn_throttled(key: &'static str, interval: Duration) -> bool {
    let Some(suppressed) = warn_throttled_with_count(key, interval) else {
        return false;
    };
    if suppressed > 0 {
        tracing::warn!(
            event = "throttled_warning_summary",
            key,
            suppressed,
            "throttled warnings were suppressed"
        );
    }
    true
}

pub fn warn_throttled_with_count(key: &'static str, interval: Duration) -> Option<u64> {
    static LAST_WARN: OnceLock<Mutex<HashMap<&'static str, WarnThrottleState>>> = OnceLock::new();
    let map = LAST_WARN.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = match map.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            tracing::warn!("warn throttle lock poisoned; continuing with recovered state");
            poisoned.into_inner()
        }
    };
    let now = Instant::now();
    if let Some(state) = guard.get_mut(key) {
        if now.saturating_duration_since(state.last) < interval {
            state.suppressed = state.suppressed.saturating_add(1);
            return None;
        }
        let suppressed = state.suppressed;
        state.last = now;
        state.suppressed = 0;
        return Some(suppressed);
    }
    guard.insert(
        key,
        WarnThrottleState {
            last: now,
            suppressed: 0,
        },
    );
    Some(0)
}
