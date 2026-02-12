use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

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
    static LAST_WARN: OnceLock<Mutex<HashMap<&'static str, Instant>>> = OnceLock::new();
    let map = LAST_WARN.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().expect("warn throttle lock poisoned");
    let now = Instant::now();
    if let Some(last) = guard.get(key)
        && now.saturating_duration_since(*last) < interval
    {
        return false;
    }
    guard.insert(key, now);
    true
}
