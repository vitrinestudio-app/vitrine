//! Fixed-window rate limit per client key (uploads, reports). In memory:
//! a restart resets it; the reverse proxy may add its own limits.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(3600);

#[derive(Debug)]
pub struct RateLimiter {
    per_window: u32,
    hits: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new(per_hour: u32) -> Self {
        Self {
            per_window: per_hour,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// Counts a hit for `key`; `false` when the limit is reached.
    pub fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut hits = self
            .hits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if hits.len() > 10_000 {
            hits.retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
        }
        let entry = hits.entry(key.to_owned()).or_insert((now, 0));
        if now.duration_since(entry.0) >= WINDOW {
            *entry = (now, 0);
        }
        if entry.1 >= self.per_window {
            return false;
        }
        entry.1 += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_per_key() {
        let l = RateLimiter::new(2);
        assert!(l.allow("a") && l.allow("a"));
        assert!(!l.allow("a"));
        assert!(l.allow("b"));
    }
}
