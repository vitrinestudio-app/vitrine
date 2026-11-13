//! Fixed-window rate limit per client key (the client's IP address). In
//! memory only: a restart resets it, and [`RateLimiter::prune`] (run every
//! few seconds by `maintenance::housekeeping`) drops a key as soon as its
//! one-hour window has passed – the privacy policy relies on that. The
//! reverse proxy may add its own limits.

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

    fn hits(&self) -> std::sync::MutexGuard<'_, HashMap<String, (Instant, u32)>> {
        self.hits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Drops every key whose window has passed.
    pub fn prune(&self) {
        self.prune_at(Instant::now());
    }

    fn prune_at(&self, now: Instant) {
        self.hits()
            .retain(|_, (start, _)| now.saturating_duration_since(*start) < WINDOW);
    }

    /// Counts a hit for `key`; `false` when the limit is reached.
    pub fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut hits = self.hits();
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

    #[test]
    fn prune_drops_expired_windows() {
        let l = RateLimiter::new(5);
        assert!(l.allow("a"));
        l.prune();
        assert_eq!(l.hits().len(), 1, "a running window is kept");
        l.prune_at(Instant::now() + WINDOW);
        assert!(l.hits().is_empty(), "an expired window is dropped");
    }
}
