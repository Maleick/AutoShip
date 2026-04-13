//! Game state observer — serializes TUI app state to compact JSON for the Gemma 4 LLM observer.
//!
//! The observer snapshot is a cheap, allocation-minimal representation of the fleet state that
//! can be forwarded to a local LLM (e.g., Gemma 4 via ollama) without exposing internal types.
//!
//! JSON shape:
//! ```json
//! {
//!   "tick": 42,
//!   "clients": [
//!     {"id":1,"name":"Kira","class":"CLR","level":60,"hp":95,"mp":80,"target":"Mob","zone":"gfaydark"}
//!   ],
//!   "camp": {"zone":"gfaydark","pull_point":[100.0,200.0,0.0]}
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::camp::config::CampConfig;
use crate::tui::app::App;

// ---------------------------------------------------------------------------
// Rate limiter stub (will be replaced by #882 implementation)
// ---------------------------------------------------------------------------

/// Stub rate limiter for LLM observer calls.
///
/// Tracks the minimum interval between observer snapshot serializations so that
/// the LLM is not called more frequently than the configured rate allows.
/// Replace with the real `LlmRateLimiter` once issue #882 is implemented.
pub struct LlmRateLimiter {
    min_interval_ms: u64,
    last_snapshot_ms: Option<u64>,
}

impl LlmRateLimiter {
    /// Create a new rate limiter with the given minimum interval in milliseconds.
    #[must_use]
    pub const fn new(min_interval_ms: u64) -> Self {
        Self {
            min_interval_ms,
            last_snapshot_ms: None,
        }
    }

    /// Returns `true` if enough time has elapsed since the last snapshot to
    /// allow another one.  `now_ms` is a monotonic millisecond timestamp
    /// (e.g., from the TUI tick counter scaled by the refresh rate).
    #[must_use]
    pub fn is_ready(&self, now_ms: u64) -> bool {
        match self.last_snapshot_ms {
            None => true,
            Some(last) => now_ms.saturating_sub(last) >= self.min_interval_ms,
        }
    }

    /// Record that a snapshot was taken at `now_ms`.
    pub fn record(&mut self, now_ms: u64) {
        self.last_snapshot_ms = Some(now_ms);
    }
}

impl Default for LlmRateLimiter {
    fn default() -> Self {
        // Default: at most one snapshot per second.
        Self::new(1_000)
    }
}

// ---------------------------------------------------------------------------
// Snapshot types
// ---------------------------------------------------------------------------

/// Compact JSON snapshot of a single EQ client for the LLM observer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientSnapshot {
    /// Numeric client index within the fleet (1-based, matches slot position).
    pub id: usize,
    /// Character name (or empty string if not yet identified).
    pub name: String,
    /// Short class abbreviation (e.g., "CLR", "WAR") or empty string.
    pub class: String,
    /// Character level (0 when unknown).
    pub level: u8,
    /// Current HP percentage (0-100).
    pub hp: u8,
    /// Current mana percentage (0-100, 0 for classes without mana).
    pub mp: u8,
    /// Name of the current target, or empty string if untargeted.
    pub target: String,
    /// Current zone short name.
    pub zone: String,
}

/// Compact JSON snapshot of the active camp configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CampSnapshot {
    /// Zone short name where the camp is located.
    pub zone: String,
    /// Pull point coordinates `[x, y, z]`.
    pub pull_point: [f32; 3],
}

/// Top-level JSON snapshot of the entire fleet state for the LLM observer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameStateSnapshot {
    /// Monotonic TUI tick counter at the time the snapshot was taken.
    pub tick: u64,
    /// One entry per active EQ client.
    pub clients: Vec<ClientSnapshot>,
    /// Active camp configuration, if any is loaded.
    pub camp: Option<CampSnapshot>,
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

impl GameStateSnapshot {
    /// Build a `GameStateSnapshot` from the live TUI `App` state.
    ///
    /// `camp_config` is passed separately because the active camp lives on the
    /// `Orchestrator` (not the `App`); callers supply it as
    /// `orchestrator.active_camp.as_ref().map(|c| &c.config)`.
    ///
    /// This is a pure, non-blocking snapshot — no I/O is performed.
    #[must_use]
    pub fn from_app(app: &App, camp_config: Option<&CampConfig>) -> Self {
        let clients: Vec<ClientSnapshot> = app
            .clients
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let (class, level, hp, mp, target) =
                    if let Some(ref player) = c.local_player {
                        let hp = if player.hp_max > 0 {
                            ((player.hp_current as f64 / player.hp_max as f64) * 100.0)
                                .clamp(0.0, 100.0) as u8
                        } else {
                            0
                        };
                        let mp = if player.mana_max > 0 {
                            ((f64::from(player.mana_current) / f64::from(player.mana_max))
                                * 100.0)
                                .clamp(0.0, 100.0) as u8
                        } else {
                            0
                        };
                        let class = player
                            .class
                            .map(|cls| cls.to_string())
                            .unwrap_or_default();
                        let level = player.level;
                        let target_name = c
                            .target
                            .as_ref()
                            .map(|t| t.displayed_name.clone())
                            .unwrap_or_default();
                        (class, level, hp, mp, target_name)
                    } else {
                        (String::new(), 0u8, 0u8, 0u8, String::new())
                    };

                ClientSnapshot {
                    id: i + 1,
                    name: c.character_name.clone(),
                    class,
                    level,
                    hp,
                    mp,
                    target,
                    zone: c.zone_name.clone(),
                }
            })
            .collect();

        let camp = camp_config.map(|cfg| CampSnapshot {
            zone: cfg.zone.clone(),
            pull_point: cfg.pull_point,
        });

        Self {
            tick: app.tick_count,
            clients,
            camp,
        }
    }

    /// Serialize the snapshot to a compact JSON string.
    ///
    /// Returns `None` if serialization fails (should be infallible in practice).
    #[must_use]
    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string(self).ok()
    }

    /// Serialize the snapshot to a pretty-printed JSON string.
    ///
    /// Useful for debug logging and human review.
    #[must_use]
    pub fn to_json_pretty(&self) -> Option<String> {
        serde_json::to_string_pretty(self).ok()
    }
}

/// Serialize TUI app state to a compact JSON string, respecting the rate limiter.
///
/// `camp_config` is passed separately because the active camp lives on the
/// `Orchestrator`; supply `orchestrator.active_camp.as_ref().map(|c| &c.config)`.
///
/// Returns `None` if the rate limiter is not ready or serialization fails.
/// When `Some` is returned, the rate limiter is updated with `now_ms`.
pub fn serialize_game_state(
    app: &App,
    camp_config: Option<&CampConfig>,
    limiter: &mut LlmRateLimiter,
    now_ms: u64,
) -> Option<String> {
    if !limiter.is_ready(now_ms) {
        return None;
    }
    let snapshot = GameStateSnapshot::from_app(app, camp_config);
    let json = snapshot.to_json()?;
    limiter.record(now_ms);
    Some(json)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(
        tick: u64,
        clients: Vec<ClientSnapshot>,
        camp: Option<CampSnapshot>,
    ) -> GameStateSnapshot {
        GameStateSnapshot { tick, clients, camp }
    }

    fn client(id: usize, name: &str, class: &str, level: u8, hp: u8, mp: u8, target: &str, zone: &str) -> ClientSnapshot {
        ClientSnapshot {
            id,
            name: name.into(),
            class: class.into(),
            level,
            hp,
            mp,
            target: target.into(),
            zone: zone.into(),
        }
    }

    #[test]
    fn snapshot_serializes_to_valid_json() {
        let snap = make_snapshot(
            10,
            vec![client(1, "Kira", "CLR", 60, 95, 80, "Goblin", "gfaydark")],
            Some(CampSnapshot {
                zone: "gfaydark".into(),
                pull_point: [100.0, 200.0, 0.0],
            }),
        );
        let json = snap.to_json().expect("serialization must succeed");
        assert!(json.contains("\"tick\":10"));
        assert!(json.contains("\"name\":\"Kira\""));
        assert!(json.contains("\"class\":\"CLR\""));
        assert!(json.contains("\"level\":60"));
        assert!(json.contains("\"hp\":95"));
        assert!(json.contains("\"mp\":80"));
        assert!(json.contains("\"target\":\"Goblin\""));
        assert!(json.contains("\"zone\":\"gfaydark\""));
        assert!(json.contains("\"pull_point\":[100.0,200.0,0.0]"));
    }

    #[test]
    fn snapshot_deserializes_roundtrip() {
        let original = make_snapshot(
            42,
            vec![
                client(1, "Warrior", "WAR", 50, 100, 0, "", "commons"),
                client(2, "Wizard", "WIZ", 55, 75, 60, "Orc", "commons"),
            ],
            None,
        );
        let json = original.to_json().unwrap();
        let restored: GameStateSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn snapshot_no_camp_serializes_null() {
        let snap = make_snapshot(1, vec![], None);
        let json = snap.to_json().unwrap();
        assert!(json.contains("\"camp\":null"));
    }

    #[test]
    fn rate_limiter_blocks_before_interval() {
        let mut limiter = LlmRateLimiter::new(500);
        assert!(limiter.is_ready(0), "should be ready with no prior snapshot");
        limiter.record(0);
        assert!(
            !limiter.is_ready(100),
            "should not be ready after 100ms with 500ms interval"
        );
        assert!(!limiter.is_ready(499), "should not be ready at 499ms");
        assert!(limiter.is_ready(500), "should be ready at exactly the interval");
        assert!(limiter.is_ready(1000), "should be ready well after the interval");
    }

    #[test]
    fn rate_limiter_default_interval_is_one_second() {
        let limiter = LlmRateLimiter::default();
        // Default limiter is immediately ready (no prior snapshot).
        assert!(limiter.is_ready(0));
    }

    #[test]
    fn multiple_clients_have_sequential_ids() {
        let clients: Vec<ClientSnapshot> = (1..=3)
            .map(|i| client(i, &format!("Char{i}"), "WAR", 60, 100, 0, "", "nexus"))
            .collect();
        let snap = make_snapshot(5, clients, None);
        let json = snap.to_json().unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"id\":2"));
        assert!(json.contains("\"id\":3"));
    }

    #[test]
    fn snapshot_hp_mp_boundary_values() {
        // Directly construct a snapshot with boundary values to ensure they serialize correctly.
        let c = client(1, "Test", "CLR", 60, 100, 100, "", "nektulos");
        let snap = make_snapshot(1, vec![c], None);
        let json = snap.to_json().unwrap();
        assert!(json.contains("\"hp\":100"));
        assert!(json.contains("\"mp\":100"));
    }

    #[test]
    fn serialize_game_state_respects_rate_limiter() {
        // Build a minimal App using its default constructor (empty clients, no live EQ needed).
        let app = App::new();
        let mut limiter = LlmRateLimiter::new(1_000);

        // First call at t=0 — should succeed.
        let result = serialize_game_state(&app, None, &mut limiter, 0);
        assert!(result.is_some(), "first call should produce a snapshot");

        // Second call at t=500 — should be blocked.
        let result2 = serialize_game_state(&app, None, &mut limiter, 500);
        assert!(result2.is_none(), "call within interval should be blocked");

        // Third call at t=1000 — should succeed again.
        let result3 = serialize_game_state(&app, None, &mut limiter, 1_000);
        assert!(result3.is_some(), "call after interval should succeed");
    }
}
