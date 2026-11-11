//! Server-side enforcement of GitHub's device-flow polling interval.
//!
//! Per device code (kept as a SHA-256 hash, in memory): the interval GitHub
//! gave (device start, raised by `slow_down`) and the time of the last poll.
//! A poll sooner than the interval is answered `slow_down` locally, without
//! contacting GitHub. The first poll of a code is always passed on; codes
//! this server did not start (e.g. after a restart) use the default
//! interval. Entries expire with the device code.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::session::hash;

/// GitHub's default interval (seconds) when none is known.
pub const DEFAULT_INTERVAL: u64 = 5;
/// Lifetime of an entry when GitHub gave none (seconds).
const DEFAULT_EXPIRY: u64 = 900;
/// Clock/network slack accepted below the interval.
const SLACK: Duration = Duration::from_millis(500);
/// Upper bound of tracked codes (oldest expired first; then the map is
/// cleared rather than grown).
const MAX_ENTRIES: usize = 50_000;

#[derive(Debug)]
struct Entry {
    interval: u64,
    last_poll: Option<Instant>,
    expires: Instant,
}

#[derive(Debug, Default)]
pub struct PollGate {
    codes: Mutex<HashMap<String, Entry>>,
}

impl PollGate {
    fn codes(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.codes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn prune(map: &mut HashMap<String, Entry>, now: Instant) {
        if map.len() >= MAX_ENTRIES {
            map.retain(|_, e| e.expires > now);
            if map.len() >= MAX_ENTRIES {
                map.clear();
            }
        }
    }

    /// A device flow was started with GitHub's `interval` and `expires_in`.
    pub fn started(&self, device_code: &str, interval: u64, expires_in: u64) {
        let now = Instant::now();
        let mut map = self.codes();
        Self::prune(&mut map, now);
        map.insert(
            hash(device_code),
            Entry {
                interval: interval.max(1),
                last_poll: None,
                expires: now + Duration::from_secs(expires_in.max(1)),
            },
        );
    }

    /// Records a poll; `Err(interval)` when it comes too early.
    pub fn poll(&self, device_code: &str) -> Result<(), u64> {
        let now = Instant::now();
        let mut map = self.codes();
        Self::prune(&mut map, now);
        let e = map.entry(hash(device_code)).or_insert_with(|| Entry {
            interval: DEFAULT_INTERVAL,
            last_poll: None,
            expires: now + Duration::from_secs(DEFAULT_EXPIRY),
        });
        if let Some(last) = e.last_poll
            && now.duration_since(last) + SLACK < Duration::from_secs(e.interval)
        {
            return Err(e.interval);
        }
        e.last_poll = Some(now);
        Ok(())
    }

    /// GitHub answered `slow_down` with a new interval.
    pub fn set_interval(&self, device_code: &str, interval: u64) {
        if let Some(e) = self.codes().get_mut(&hash(device_code)) {
            e.interval = interval.max(e.interval);
        }
    }

    /// The flow ended (token issued or a final error).
    pub fn forget(&self, device_code: &str) {
        self.codes().remove(&hash(device_code));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_the_interval_per_code() {
        let g = PollGate::default();
        g.started("a", 5, 900);
        assert_eq!(g.poll("a"), Ok(()));
        assert_eq!(g.poll("a"), Err(5));
        // Other codes are independent; unknown codes get the default.
        assert_eq!(g.poll("b"), Ok(()));
        assert_eq!(g.poll("b"), Err(DEFAULT_INTERVAL));
        g.set_interval("a", 10);
        assert_eq!(g.poll("a"), Err(10));
        g.forget("a");
        assert_eq!(g.poll("a"), Ok(()));
    }

    #[test]
    fn short_intervals_pass_after_waiting() {
        let g = PollGate::default();
        g.started("c", 1, 60);
        assert_eq!(g.poll("c"), Ok(()));
        std::thread::sleep(Duration::from_millis(600));
        assert_eq!(g.poll("c"), Ok(()));
    }
}
