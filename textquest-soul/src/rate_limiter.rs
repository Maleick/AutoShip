//! LLM request rate limiting — global and per-character sliding-window throttles.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use serde::Deserialize;

/// Duration of the sliding window in seconds.
const WINDOW_SECS: u64 = 60;

/// Configuration for the LLM rate limiter (deserializable from TOML/JSON).
#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests allowed across all characters per minute.
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: u32,
    /// Maximum requests allowed per individual character per minute.
    #[serde(default = "default_per_character_limit")]
    pub per_character_limit: u32,
}

fn default_requests_per_minute() -> u32 {
    10
}

fn default_per_character_limit() -> u32 {
    3
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: default_requests_per_minute(),
            per_character_limit: default_per_character_limit(),
        }
    }
}

/// Sliding-window rate limiter for LLM requests.
///
/// Enforces two independent limits:
/// - A global limit across all characters (`requests_per_minute`)
/// - A per-character limit (`per_character_limit`)
///
/// Both windows are 60 seconds wide and slide continuously.
pub struct LlmRateLimiter {
    /// Maximum requests per minute across all characters.
    pub requests_per_minute: u32,
    /// Maximum requests per minute for a single character.
    pub per_character_limit: u32,
    /// Timestamps of all recent global requests within the window.
    global_window: VecDeque<Instant>,
    /// Per-character timestamps of recent requests within the window.
    character_windows: HashMap<String, VecDeque<Instant>>,
}

impl LlmRateLimiter {
    /// Create a new rate limiter from config.
    #[must_use]
    pub fn new(config: &RateLimitConfig) -> Self {
        Self {
            requests_per_minute: config.requests_per_minute,
            per_character_limit: config.per_character_limit,
            global_window: VecDeque::new(),
            character_windows: HashMap::new(),
        }
    }

    /// Create a rate limiter with explicit limits (useful for tests).
    #[must_use]
    pub fn with_limits(requests_per_minute: u32, per_character_limit: u32) -> Self {
        Self {
            requests_per_minute,
            per_character_limit,
            global_window: VecDeque::new(),
            character_windows: HashMap::new(),
        }
    }

    /// Check whether a request for `character` is allowed at `now`.
    ///
    /// If allowed, records the request in both the global window and the
    /// character's window and returns `true`. Returns `false` if either
    /// the global limit or the per-character limit would be exceeded.
    pub fn check_and_record(&mut self, character: &str, now: Instant) -> bool {
        self.drain_expired(now);

        // Check global limit
        if self.global_window.len() as u32 >= self.requests_per_minute {
            return false;
        }

        // Check per-character limit
        let char_window = self
            .character_windows
            .entry(character.to_owned())
            .or_default();
        if char_window.len() as u32 >= self.per_character_limit {
            return false;
        }

        // Record in both windows
        self.global_window.push_back(now);
        self.character_windows
            .entry(character.to_owned())
            .or_default()
            .push_back(now);

        true
    }

    /// Remove all entries older than 60 seconds from all windows.
    pub fn drain_expired(&mut self, now: Instant) {
        drain_window(&mut self.global_window, now);
        for window in self.character_windows.values_mut() {
            drain_window(window, now);
        }
    }
}

/// Remove entries from `window` that are older than `WINDOW_SECS` relative to `now`.
fn drain_window(window: &mut VecDeque<Instant>, now: Instant) {
    while let Some(&front) = window.front() {
        if now.duration_since(front).as_secs() >= WINDOW_SECS {
            window.pop_front();
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    /// Advance an `Instant` by the given number of seconds.
    fn advance(base: Instant, secs: u64) -> Instant {
        base + Duration::from_secs(secs)
    }

    #[test]
    fn global_limit_enforced() {
        let mut limiter = LlmRateLimiter::with_limits(3, 10);
        let t0 = Instant::now();

        assert!(limiter.check_and_record("Alice", t0));
        assert!(limiter.check_and_record("Bob", t0));
        assert!(limiter.check_and_record("Carol", t0));
        // 4th request exceeds global limit of 3
        assert!(!limiter.check_and_record("Dave", t0));
    }

    #[test]
    fn per_character_limit_enforced() {
        let mut limiter = LlmRateLimiter::with_limits(100, 2);
        let t0 = Instant::now();

        assert!(limiter.check_and_record("Alice", t0));
        assert!(limiter.check_and_record("Alice", t0));
        // 3rd request from Alice exceeds per-character limit of 2
        assert!(!limiter.check_and_record("Alice", t0));
        // Bob is unaffected
        assert!(limiter.check_and_record("Bob", t0));
    }

    #[test]
    fn expired_entries_drain_correctly() {
        let mut limiter = LlmRateLimiter::with_limits(2, 2);
        let t0 = Instant::now();

        // Fill the global limit
        assert!(limiter.check_and_record("Alice", t0));
        assert!(limiter.check_and_record("Alice", t0));
        assert!(!limiter.check_and_record("Alice", t0));

        // After 60 seconds the window expires — requests should be allowed again
        let t1 = advance(t0, 60);
        assert!(limiter.check_and_record("Alice", t1));
    }

    #[test]
    fn character_under_limit_not_blocked_when_global_has_room() {
        let mut limiter = LlmRateLimiter::with_limits(10, 3);
        let t0 = Instant::now();

        // Use some global capacity with Alice
        assert!(limiter.check_and_record("Alice", t0));
        assert!(limiter.check_and_record("Alice", t0));

        // Bob still has room (global not full, Bob's window empty)
        assert!(limiter.check_and_record("Bob", t0));
        assert!(limiter.check_and_record("Bob", t0));
        assert!(limiter.check_and_record("Bob", t0));
        // Bob hits per-character limit, but global still has room
        assert!(!limiter.check_and_record("Bob", t0));
        // Carol is unaffected
        assert!(limiter.check_and_record("Carol", t0));
    }

    #[test]
    fn drain_expired_removes_old_entries() {
        let mut limiter = LlmRateLimiter::with_limits(5, 5);
        let t0 = Instant::now();

        limiter.check_and_record("Alice", t0);
        limiter.check_and_record("Bob", t0);

        assert_eq!(limiter.global_window.len(), 2);

        // Drain at t0 + 60s — both entries are exactly at expiry boundary and removed
        let t1 = advance(t0, 60);
        limiter.drain_expired(t1);

        assert_eq!(limiter.global_window.len(), 0);
        assert_eq!(limiter.character_windows["Alice"].len(), 0);
        assert_eq!(limiter.character_windows["Bob"].len(), 0);
    }

    #[test]
    fn zero_limits_block_all_requests() {
        let mut limiter = LlmRateLimiter::with_limits(0, 0);
        let t0 = Instant::now();
        assert!(!limiter.check_and_record("Alice", t0));
    }

    #[test]
    fn mixed_old_and_new_entries_partial_drain() {
        let mut limiter = LlmRateLimiter::with_limits(10, 10);
        let t0 = Instant::now();

        // Two old requests
        limiter.check_and_record("Alice", t0);
        limiter.check_and_record("Alice", t0);

        // One newer request (30 seconds later — still in window at t0+60)
        let t1 = advance(t0, 30);
        limiter.check_and_record("Alice", t1);

        // At t0 + 60, the two old entries expire but the t1 entry stays
        let t2 = advance(t0, 60);
        limiter.drain_expired(t2);

        assert_eq!(limiter.character_windows["Alice"].len(), 1);
    }

    // --- Additional rate limiter tests ---

    #[test]
    fn default_config_values() {
        let cfg = RateLimitConfig::default();
        assert_eq!(cfg.requests_per_minute, 10);
        assert_eq!(cfg.per_character_limit, 3);
    }

    #[test]
    fn from_config_uses_config_values() {
        let cfg = RateLimitConfig {
            requests_per_minute: 20,
            per_character_limit: 5,
        };
        let limiter = LlmRateLimiter::new(&cfg);
        assert_eq!(limiter.requests_per_minute, 20);
        assert_eq!(limiter.per_character_limit, 5);
    }

    #[test]
    fn one_request_per_minute_allows_one() {
        let mut limiter = LlmRateLimiter::with_limits(1, 1);
        let t0 = Instant::now();
        assert!(limiter.check_and_record("Alice", t0));
        assert!(!limiter.check_and_record("Alice", t0));
        assert!(!limiter.check_and_record("Bob", t0)); // global also full
    }

    #[test]
    fn requests_allowed_after_full_window_expiry() {
        let mut limiter = LlmRateLimiter::with_limits(2, 2);
        let t0 = Instant::now();

        limiter.check_and_record("Alice", t0);
        limiter.check_and_record("Alice", t0);
        assert!(!limiter.check_and_record("Alice", t0)); // full

        // 61 seconds later — entire window expired
        let t1 = advance(t0, 61);
        assert!(limiter.check_and_record("Alice", t1));
        assert!(limiter.check_and_record("Alice", t1));
    }

    #[test]
    fn global_limit_blocks_new_characters() {
        let mut limiter = LlmRateLimiter::with_limits(2, 10);
        let t0 = Instant::now();

        limiter.check_and_record("Alice", t0);
        limiter.check_and_record("Bob", t0);
        // Global limit 2 reached
        assert!(!limiter.check_and_record("Carol", t0));
    }

    #[test]
    fn per_char_limit_independent_of_others() {
        let mut limiter = LlmRateLimiter::with_limits(100, 1);
        let t0 = Instant::now();

        assert!(limiter.check_and_record("Alice", t0));
        assert!(!limiter.check_and_record("Alice", t0)); // per-char limit

        // Bob is independent
        assert!(limiter.check_and_record("Bob", t0));
        assert!(!limiter.check_and_record("Bob", t0));
    }

    #[test]
    fn sliding_window_boundary_entry_exactly_at_60s() {
        let mut limiter = LlmRateLimiter::with_limits(2, 2);
        let t0 = Instant::now();

        limiter.check_and_record("Alice", t0);

        // At exactly 60 seconds, the entry expires (>= WINDOW_SECS)
        let t1 = advance(t0, 60);
        limiter.drain_expired(t1);

        assert_eq!(limiter.global_window.len(), 0);
    }

    #[test]
    fn drain_expired_on_empty_limiter_is_safe() {
        let mut limiter = LlmRateLimiter::with_limits(10, 10);
        let t0 = Instant::now();
        limiter.drain_expired(t0);
        assert!(limiter.global_window.is_empty());
    }
}
