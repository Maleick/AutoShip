//! Clicky item automation — cooldown tracking, condition evaluation, and
//! per-character item-use dispatch.
//!
//! # Design
//!
//! Each [`ClickyItem`] describes a clickable item: which inventory slot it
//! lives in, how long its cooldown is, and under what HP/mana/buff conditions
//! it should fire. [`ClickyManager`] owns a list of items and a cooldown map
//! keyed by `(character_id, item_name)`. On every call to [`ClickyManager::tick`]
//! it evaluates conditions, respects cooldowns, and sends `/useitem` slash
//! commands via the IPC channel.
//!
//! # DLL abstraction
//!
//! Item clicks are dispatched through a [`ClickExecutor`] trait so that unit
//! tests can inject a [`StubExecutor`] instead of requiring a live DLL
//! connection. The production path sends a `/useitem <slot>` slash command
//! via `mpsc::Sender<IpcCommand>`.

#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::too_long_first_doc_paragraph
)]

use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use textquest_common::ipc::{Command, IpcCommand};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Default ring-buffer capacity for per-manager usage history.
const DEFAULT_HISTORY_CAP: usize = 64;

// ── ClickCondition ────────────────────────────────────────────────────────────

/// Condition that must be satisfied before a clicky item fires.
///
/// All specified conditions must be true simultaneously (logical AND).
/// An item with no conditions fires whenever its cooldown has expired.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClickCondition {
    /// Fire only when HP% is at or below this threshold (0–100).
    /// `None` = no HP restriction.
    pub hp_pct_max: Option<f32>,
    /// Fire only when mana% is at or below this threshold (0–100).
    /// `None` = no mana restriction.
    pub mana_pct_max: Option<f32>,
    /// Fire only when the character does NOT currently have any of these buffs.
    /// Empty = no buff restriction.
    #[serde(default)]
    pub missing_buffs: Vec<String>,
}

impl ClickCondition {
    /// Returns true if all conditions in `self` are satisfied given the current
    /// game state snapshot.
    #[must_use]
    pub fn is_satisfied(&self, snapshot: &CharSnapshot) -> bool {
        if self.hp_pct_max.is_some_and(|max_hp| snapshot.hp_pct > max_hp) {
            return false;
        }
        if self.mana_pct_max.is_some_and(|max_mana| snapshot.mana_pct > max_mana) {
            return false;
        }
        for buff in &self.missing_buffs {
            if snapshot
                .active_buffs
                .iter()
                .any(|b| b.eq_ignore_ascii_case(buff))
            {
                return false; // buff is present — do not fire
            }
        }
        true
    }
}

// ── CharSnapshot ──────────────────────────────────────────────────────────────

/// Lightweight per-character game-state snapshot used for condition evaluation.
#[derive(Debug, Clone)]
pub struct CharSnapshot {
    /// Current HP as a percentage (0.0–100.0).
    pub hp_pct: f32,
    /// Current mana as a percentage (0.0–100.0).
    pub mana_pct: f32,
    /// Names of buffs currently active on the character.
    pub active_buffs: Vec<String>,
}

impl Default for CharSnapshot {
    fn default() -> Self {
        Self {
            hp_pct: 100.0,
            mana_pct: 100.0,
            active_buffs: Vec::new(),
        }
    }
}

// ── ClickyItem ────────────────────────────────────────────────────────────────

/// A single clickable item that the automation system can use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClickyItem {
    /// Display name / identifier (e.g. "Journeyman's Boots", "Fungi Staff").
    pub name: String,
    /// EQ inventory slot number (0-based, matches `/useitem <slot>`).
    /// Common slots: 0 = primary, 1 = secondary, 13 = ammo, 22 = charm.
    pub slot: u8,
    /// Milliseconds between uses (the item's recast / cooldown time).
    pub cooldown_ms: u64,
    /// Conditions that must all be satisfied before the item fires.
    /// Empty conditions list means: fire whenever the cooldown has expired.
    #[serde(default)]
    pub conditions: Vec<ClickCondition>,
    /// Priority for queue ordering. Lower numbers fire first when multiple
    /// items are ready simultaneously (0 = highest priority).
    #[serde(default)]
    pub priority: u8,
}

impl ClickyItem {
    /// Returns true when all conditions are met for a given snapshot.
    #[must_use]
    pub fn conditions_met(&self, snapshot: &CharSnapshot) -> bool {
        self.conditions.iter().all(|c| c.is_satisfied(snapshot))
    }
}

// ── ClickExecutor trait ───────────────────────────────────────────────────────

/// Abstraction over the DLL dispatch mechanism.
///
/// The production implementation sends a `/useitem <slot>` slash command via
/// an `mpsc::Sender<IpcCommand>`. Tests inject a [`StubExecutor`] that records
/// calls instead of touching IPC.
pub trait ClickExecutor {
    /// Request that the item in `slot` be used for `character_id`.
    ///
    /// The executor may fire-and-forget or buffer the command; callers do not
    /// inspect the result beyond logging.
    fn execute(&self, character_id: u32, item: &ClickyItem) -> bool;
}

// ── IpcExecutor (production) ──────────────────────────────────────────────────

/// Production executor — sends `/useitem <slot>` through the IPC channel.
pub struct IpcExecutor {
    ipc_tx: std::sync::mpsc::Sender<IpcCommand>,
}

impl IpcExecutor {
    /// Wrap an existing IPC sender.
    #[must_use]
    pub fn new(ipc_tx: std::sync::mpsc::Sender<IpcCommand>) -> Self {
        Self { ipc_tx }
    }
}

impl ClickExecutor for IpcExecutor {
    fn execute(&self, _character_id: u32, item: &ClickyItem) -> bool {
        let cmd = IpcCommand::new(Command::SlashCommand {
            command: format!("/useitem {}", item.slot),
        });
        self.ipc_tx.send(cmd).is_ok()
    }
}

// ── StubExecutor (tests) ──────────────────────────────────────────────────────

/// Test stub that records every click request without touching IPC.
#[derive(Debug, Default)]
pub struct StubExecutor {
    /// Recorded `(character_id, item_name, slot)` tuples in call order.
    pub calls: std::cell::RefCell<Vec<(u32, String, u8)>>,
}

impl ClickExecutor for StubExecutor {
    fn execute(&self, character_id: u32, item: &ClickyItem) -> bool {
        self.calls
            .borrow_mut()
            .push((character_id, item.name.clone(), item.slot));
        true
    }
}

// ── ClickEvent ────────────────────────────────────────────────────────────────

/// A record of a single clicky usage kept in the ring buffer.
#[derive(Debug, Clone)]
pub struct ClickEvent {
    /// Wall-clock instant when the click was dispatched.
    pub at: Instant,
    /// Character that used the item.
    pub character_id: u32,
    /// Name of the item used.
    pub item_name: String,
    /// Slot the item occupied.
    pub slot: u8,
}

// ── ClickyManager ─────────────────────────────────────────────────────────────

/// Drives clicky item automation for one or more characters.
///
/// # Usage
///
/// 1. Build with [`ClickyManager::new`], passing the list of configured items.
/// 2. Call [`ClickyManager::tick`] on every orchestrator tick, providing
///    the current [`Instant`], a map of per-character snapshots, and an
///    executor (production: [`IpcExecutor`]; tests: [`StubExecutor`]).
/// 3. Inspect [`ClickyManager::history`] for audit / TUI display.
pub struct ClickyManager {
    /// Configured items, sorted by priority ascending on construction.
    pub items: Vec<ClickyItem>,
    /// `(character_id, item_name)` → last-used [`Instant`].
    cooldowns: HashMap<(u32, String), Instant>,
    /// Ring buffer of the last N click events.
    pub history: VecDeque<ClickEvent>,
    /// Maximum entries kept in `history`.
    history_cap: usize,
    /// Total number of clicks dispatched this session.
    pub total_clicks: u64,
}

impl ClickyManager {
    /// Create a manager from a list of items.
    ///
    /// Items are sorted by `priority` (ascending) so that higher-priority items
    /// are evaluated first on each tick.
    #[must_use]
    pub fn new(mut items: Vec<ClickyItem>) -> Self {
        items.sort_by_key(|i| i.priority);
        Self {
            items,
            cooldowns: HashMap::new(),
            history: VecDeque::new(),
            history_cap: DEFAULT_HISTORY_CAP,
            total_clicks: 0,
        }
    }

    /// Override the history ring-buffer capacity (default: 64).
    pub fn with_history_cap(mut self, cap: usize) -> Self {
        self.history_cap = cap;
        self
    }

    // ── Cooldown helpers ──────────────────────────────────────────────────────

    /// Returns `true` when `item`'s cooldown for `character_id` has elapsed.
    #[must_use]
    pub fn is_ready(&self, character_id: u32, item: &ClickyItem, now: Instant) -> bool {
        match self.cooldowns.get(&(character_id, item.name.clone())) {
            None => true,
            Some(&last) => now.duration_since(last).as_millis() >= u128::from(item.cooldown_ms),
        }
    }

    /// Returns the milliseconds remaining on `item`'s cooldown for
    /// `character_id`, or `0` if the item is already ready.
    #[must_use]
    pub fn cooldown_remaining_ms(
        &self,
        character_id: u32,
        item: &ClickyItem,
        now: Instant,
    ) -> u64 {
        match self.cooldowns.get(&(character_id, item.name.clone())) {
            None => 0,
            Some(&last) => {
                let elapsed = now.duration_since(last).as_millis() as u64;
                item.cooldown_ms.saturating_sub(elapsed)
            }
        }
    }

    // ── Queue / tick ──────────────────────────────────────────────────────────

    /// Collect items that are ready to fire for a single character.
    ///
    /// Returns item references sorted by priority. The caller can further
    /// filter or limit the list before dispatching.
    #[must_use]
    pub fn ready_items(
        &self,
        character_id: u32,
        snapshot: &CharSnapshot,
        now: Instant,
    ) -> Vec<&ClickyItem> {
        self.items
            .iter()
            .filter(|item| {
                self.is_ready(character_id, item, now) && item.conditions_met(snapshot)
            })
            .collect()
    }

    /// Drive the clicky loop for one tick.
    ///
    /// For every `(character_id, snapshot)` pair in `char_states`, iterates
    /// items in priority order. Each item that is ready and whose conditions
    /// are met is clicked via `executor`. Cooldowns are updated and events are
    /// appended to the ring buffer.
    ///
    /// Returns a list of log messages (one per click dispatched).
    pub fn tick<E: ClickExecutor>(
        &mut self,
        now: Instant,
        char_states: &[(u32, CharSnapshot)],
        executor: &E,
    ) -> Vec<String> {
        let mut log = Vec::new();

        for (character_id, snapshot) in char_states {
            let character_id = *character_id;

            // Collect ready items (borrow items immutably before mutation).
            let ready: Vec<usize> = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    self.is_ready(character_id, item, now)
                        && item.conditions_met(snapshot)
                })
                .map(|(idx, _)| idx)
                .collect();

            for idx in ready {
                let item = &self.items[idx];
                let dispatched = executor.execute(character_id, item);

                if dispatched {
                    let msg = format!(
                        "clickies: char={character_id} used '{}' slot={} (click #{})",
                        item.name,
                        item.slot,
                        self.total_clicks + 1
                    );
                    tracing::info!(
                        character_id,
                        item_name = %item.name,
                        slot = item.slot,
                        "clicky item used"
                    );

                    // Update cooldown.
                    self.cooldowns
                        .insert((character_id, item.name.clone()), now);

                    // Append to history ring buffer.
                    let event = ClickEvent {
                        at: now,
                        character_id,
                        item_name: item.name.clone(),
                        slot: item.slot,
                    };
                    self.history.push_back(event);
                    if self.history.len() > self.history_cap {
                        self.history.pop_front();
                    }

                    self.total_clicks += 1;
                    log.push(msg);
                }
            }
        }

        log
    }

    /// Force-reset the cooldown for a specific item/character pair.
    ///
    /// Useful when the orchestrator knows the item was already used externally
    /// (e.g., the player clicked it manually).
    pub fn record_external_use(&mut self, character_id: u32, item_name: &str, at: Instant) {
        self.cooldowns
            .insert((character_id, item_name.to_string()), at);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    // ── helpers ───────────────────────────────────────────────────────────────

    fn simple_item(name: &str, slot: u8, cooldown_ms: u64) -> ClickyItem {
        ClickyItem {
            name: name.to_string(),
            slot,
            cooldown_ms,
            conditions: vec![],
            priority: 0,
        }
    }

    fn healthy_snapshot() -> CharSnapshot {
        CharSnapshot {
            hp_pct: 100.0,
            mana_pct: 100.0,
            active_buffs: vec![],
        }
    }

    // ── ClickCondition tests ──────────────────────────────────────────────────

    #[test]
    fn condition_hp_max_blocks_at_high_hp() {
        let cond = ClickCondition {
            hp_pct_max: Some(50.0),
            mana_pct_max: None,
            missing_buffs: vec![],
        };
        let snap = CharSnapshot {
            hp_pct: 75.0,
            ..Default::default()
        };
        assert!(!cond.is_satisfied(&snap), "should block when hp > max");
    }

    #[test]
    fn condition_hp_max_allows_at_low_hp() {
        let cond = ClickCondition {
            hp_pct_max: Some(50.0),
            mana_pct_max: None,
            missing_buffs: vec![],
        };
        let snap = CharSnapshot {
            hp_pct: 40.0,
            ..Default::default()
        };
        assert!(cond.is_satisfied(&snap));
    }

    #[test]
    fn condition_mana_max_blocks_at_full_mana() {
        let cond = ClickCondition {
            hp_pct_max: None,
            mana_pct_max: Some(30.0),
            missing_buffs: vec![],
        };
        let snap = CharSnapshot {
            mana_pct: 80.0,
            ..Default::default()
        };
        assert!(!cond.is_satisfied(&snap));
    }

    #[test]
    fn condition_missing_buff_blocks_when_buff_present() {
        let cond = ClickCondition {
            hp_pct_max: None,
            mana_pct_max: None,
            missing_buffs: vec!["Spirit of Wolf".to_string()],
        };
        let snap = CharSnapshot {
            active_buffs: vec!["Spirit of Wolf".to_string()],
            ..Default::default()
        };
        assert!(!cond.is_satisfied(&snap), "buff is present — should block");
    }

    #[test]
    fn condition_missing_buff_allows_when_buff_absent() {
        let cond = ClickCondition {
            hp_pct_max: None,
            mana_pct_max: None,
            missing_buffs: vec!["Spirit of Wolf".to_string()],
        };
        let snap = CharSnapshot {
            active_buffs: vec![],
            ..Default::default()
        };
        assert!(cond.is_satisfied(&snap));
    }

    #[test]
    fn condition_missing_buff_case_insensitive() {
        let cond = ClickCondition {
            hp_pct_max: None,
            mana_pct_max: None,
            missing_buffs: vec!["spirit of wolf".to_string()],
        };
        let snap = CharSnapshot {
            active_buffs: vec!["Spirit of Wolf".to_string()],
            ..Default::default()
        };
        assert!(!cond.is_satisfied(&snap));
    }

    // ── Ready detection ───────────────────────────────────────────────────────

    #[test]
    fn item_ready_with_no_prior_use() {
        let mgr = ClickyManager::new(vec![simple_item("Boots", 0, 1_000)]);
        let now = Instant::now();
        assert!(mgr.is_ready(1, &mgr.items[0], now));
    }

    #[test]
    fn item_not_ready_before_cooldown_expires() {
        let mut mgr = ClickyManager::new(vec![simple_item("Boots", 0, 5_000)]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();
        mgr.tick(t0, &[(1, healthy_snapshot())], &stub);

        // Immediately after use, not yet ready.
        let t1 = t0 + Duration::from_millis(100);
        assert!(!mgr.is_ready(1, &mgr.items[0], t1));
    }

    #[test]
    fn item_ready_after_cooldown_expires() {
        let mut mgr = ClickyManager::new(vec![simple_item("Boots", 0, 1_000)]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();
        mgr.tick(t0, &[(1, healthy_snapshot())], &stub);

        // After full cooldown elapsed, should be ready again.
        let t1 = t0 + Duration::from_millis(1_001);
        assert!(mgr.is_ready(1, &mgr.items[0], t1));
    }

    // ── Cooldown enforcement ──────────────────────────────────────────────────

    #[test]
    fn cooldown_blocks_double_click_same_tick() {
        let mut mgr = ClickyManager::new(vec![simple_item("Boots", 0, 10_000)]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();

        mgr.tick(t0, &[(1, healthy_snapshot())], &stub);
        mgr.tick(t0, &[(1, healthy_snapshot())], &stub);

        let calls = stub.calls.borrow();
        assert_eq!(calls.len(), 1, "second click same tick should be blocked");
    }

    #[test]
    fn cooldown_remaining_ms_returns_zero_when_ready() {
        let mgr = ClickyManager::new(vec![simple_item("Boots", 0, 1_000)]);
        let now = Instant::now();
        assert_eq!(mgr.cooldown_remaining_ms(1, &mgr.items[0], now), 0);
    }

    #[test]
    fn cooldown_remaining_ms_nonzero_after_use() {
        let mut mgr = ClickyManager::new(vec![simple_item("Boots", 0, 5_000)]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();
        mgr.tick(t0, &[(1, healthy_snapshot())], &stub);

        let t1 = t0 + Duration::from_millis(1_000);
        let remaining = mgr.cooldown_remaining_ms(1, &mgr.items[0], t1);
        assert!(remaining > 0, "should have cooldown remaining");
        assert!(remaining <= 5_000);
    }

    // ── Queue priority ────────────────────────────────────────────────────────

    #[test]
    fn items_fired_in_priority_order() {
        let high = ClickyItem {
            priority: 0,
            ..simple_item("HighPri", 0, 0)
        };
        let low = ClickyItem {
            priority: 1,
            ..simple_item("LowPri", 1, 0)
        };
        let mut mgr = ClickyManager::new(vec![low, high]); // intentionally reversed
        let t0 = Instant::now();
        let stub = StubExecutor::default();

        mgr.tick(t0, &[(1, healthy_snapshot())], &stub);

        let calls = stub.calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].1, "HighPri", "priority=0 should fire first");
        assert_eq!(calls[1].1, "LowPri");
    }

    // ── Condition gating ──────────────────────────────────────────────────────

    #[test]
    fn item_skipped_when_conditions_not_met() {
        let item = ClickyItem {
            conditions: vec![ClickCondition {
                hp_pct_max: Some(50.0),
                mana_pct_max: None,
                missing_buffs: vec![],
            }],
            ..simple_item("HealClicky", 5, 0)
        };
        let mut mgr = ClickyManager::new(vec![item]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();
        let snap = CharSnapshot {
            hp_pct: 90.0,
            ..Default::default()
        };

        mgr.tick(t0, &[(1, snap)], &stub);
        assert!(stub.calls.borrow().is_empty(), "should not fire at 90% HP");
    }

    #[test]
    fn item_fires_when_conditions_met() {
        let item = ClickyItem {
            conditions: vec![ClickCondition {
                hp_pct_max: Some(50.0),
                mana_pct_max: None,
                missing_buffs: vec![],
            }],
            ..simple_item("HealClicky", 5, 0)
        };
        let mut mgr = ClickyManager::new(vec![item]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();
        let snap = CharSnapshot {
            hp_pct: 30.0,
            ..Default::default()
        };

        mgr.tick(t0, &[(1, snap)], &stub);
        assert_eq!(stub.calls.borrow().len(), 1);
    }

    // ── History capture ───────────────────────────────────────────────────────

    #[test]
    fn history_records_click_events() {
        let mut mgr = ClickyManager::new(vec![simple_item("Boots", 0, 0)]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();

        mgr.tick(t0, &[(1, healthy_snapshot())], &stub);

        assert_eq!(mgr.history.len(), 1);
        assert_eq!(mgr.history[0].item_name, "Boots");
        assert_eq!(mgr.history[0].character_id, 1);
    }

    #[test]
    fn history_capped_at_configured_size() {
        let mut mgr = ClickyManager::new(vec![simple_item("Boots", 0, 0)])
            .with_history_cap(3);
        let stub = StubExecutor::default();

        // Fire 5 times using advancing time to reset cooldown.
        for i in 0..5u64 {
            let t = Instant::now() + Duration::from_secs(i);
            mgr.tick(t, &[(1, healthy_snapshot())], &stub);
        }

        assert!(
            mgr.history.len() <= 3,
            "history should be capped at 3, got {}",
            mgr.history.len()
        );
    }

    #[test]
    fn total_clicks_increments_correctly() {
        let mut mgr = ClickyManager::new(vec![
            simple_item("ItemA", 0, 0),
            simple_item("ItemB", 1, 0),
        ]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();

        mgr.tick(t0, &[(1, healthy_snapshot()), (2, healthy_snapshot())], &stub);

        // 2 items × 2 characters = 4 clicks
        assert_eq!(mgr.total_clicks, 4);
    }

    // ── Multi-character isolation ─────────────────────────────────────────────

    #[test]
    fn cooldowns_are_isolated_per_character() {
        let mut mgr = ClickyManager::new(vec![simple_item("Boots", 0, 60_000)]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();

        // char 1 uses the item
        mgr.tick(t0, &[(1, healthy_snapshot())], &stub);

        // char 2 should still be able to use it
        mgr.tick(t0, &[(2, healthy_snapshot())], &stub);

        let calls = stub.calls.borrow();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().any(|(cid, _, _)| *cid == 1));
        assert!(calls.iter().any(|(cid, _, _)| *cid == 2));
    }

    // ── External use recording ────────────────────────────────────────────────

    #[test]
    fn record_external_use_enforces_cooldown() {
        let mut mgr = ClickyManager::new(vec![simple_item("Boots", 0, 60_000)]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();

        // Simulate the player clicking the item manually.
        mgr.record_external_use(1, "Boots", t0);

        // Should now be blocked.
        mgr.tick(t0, &[(1, healthy_snapshot())], &stub);
        assert!(stub.calls.borrow().is_empty(), "external use should enforce cooldown");
    }

    // ── Log output ────────────────────────────────────────────────────────────

    #[test]
    fn tick_returns_log_messages_on_click() {
        let mut mgr = ClickyManager::new(vec![simple_item("Boots", 0, 0)]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();

        let logs = mgr.tick(t0, &[(1, healthy_snapshot())], &stub);
        assert_eq!(logs.len(), 1);
        assert!(logs[0].contains("Boots"));
        assert!(logs[0].contains("char=1"));
    }

    #[test]
    fn tick_returns_empty_log_when_nothing_fires() {
        let item = ClickyItem {
            conditions: vec![ClickCondition {
                hp_pct_max: Some(10.0),
                mana_pct_max: None,
                missing_buffs: vec![],
            }],
            ..simple_item("Boots", 0, 0)
        };
        let mut mgr = ClickyManager::new(vec![item]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();
        let snap = CharSnapshot {
            hp_pct: 100.0,
            ..Default::default()
        };

        let logs = mgr.tick(t0, &[(1, snap)], &stub);
        assert!(logs.is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TOML Configuration Layer (#1040)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Adds a TOML-based config system on top of the core clicky automation above.
//
// # Layout
//
//   config/clickies/
//     global.toml        ← applies to every character
//     Kira.toml          ← character-specific overrides / additions
//     Warrior01.toml
//
// Each file holds a `[[clicky]]` array of [`ClickyItemToml`] entries. On
// [`ClickyConfigLoader::load`], global items are merged first then character
// items appended, and the combined set is converted to [`ClickyItem`] structs
// for use with [`ClickyManager`].
//
// # Hot-reload
//
// Call [`ClickyConfigLoader::reload_if_changed`] periodically (e.g. every
// second) from the camp loop. The loader tracks the max mtime of all `.toml`
// files in the config directory and re-reads when a change is detected.

pub mod config {
    use super::{ClickCondition, ClickyItem};
    use anyhow::{Context, Result};
    use serde::{Deserialize, Serialize};
    use std::{
        path::{Path, PathBuf},
        time::{Duration, SystemTime},
    };

    // ── Per-item TOML schema ──────────────────────────────────────────────────

    /// TOML schema for a single clicky item condition.
    ///
    /// Maps directly to [`ClickCondition`] field names.
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    pub struct ClickConditionToml {
        /// Fire only when HP% is at or below this value. `None` = no HP gate.
        #[serde(default)]
        pub hp_pct_max: Option<f32>,
        /// Fire only when mana% is at or below this value. `None` = no mana gate.
        #[serde(default)]
        pub mana_pct_max: Option<f32>,
        /// Fire only when the character does NOT have any of these buffs.
        #[serde(default)]
        pub missing_buffs: Vec<String>,
    }

    impl From<ClickConditionToml> for ClickCondition {
        fn from(t: ClickConditionToml) -> Self {
            Self {
                hp_pct_max: t.hp_pct_max,
                mana_pct_max: t.mana_pct_max,
                missing_buffs: t.missing_buffs,
            }
        }
    }

    /// TOML schema for a single clicky item.
    ///
    /// All fields except `name` and `slot` are optional with sensible defaults.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct ClickyItemToml {
        /// Human-readable label (e.g. `"Journeyman's Boots"`).
        pub name: String,
        /// EQ inventory slot number (0-based, matches `/useitem <slot>`).
        pub slot: u8,
        /// Milliseconds between uses. Defaults to 0 (no cooldown enforced).
        #[serde(default)]
        pub cooldown_ms: u64,
        /// Lower value fires first (0 = highest priority). Defaults to 0.
        #[serde(default)]
        pub priority: u8,
        /// Whether this clicky is active. Defaults to `true`.
        #[serde(default = "default_true")]
        pub enabled: bool,
        /// Conditions — all must hold simultaneously. Empty = always fire.
        #[serde(default)]
        pub conditions: Vec<ClickConditionToml>,
        /// Optional operator note / description.
        #[serde(default)]
        pub note: Option<String>,
        /// Combat-only: when `true` this item is skipped outside of combat.
        /// Convenience shorthand; equivalent to a `missing_buffs`-free in-combat
        /// condition gate handled by the caller.
        #[serde(default)]
        pub combat_only: bool,
        /// Out-of-combat-only: skip this item during combat.
        #[serde(default)]
        pub ooc_only: bool,
    }

    fn default_true() -> bool {
        true
    }

    impl From<ClickyItemToml> for ClickyItem {
        fn from(t: ClickyItemToml) -> Self {
            Self {
                name: t.name,
                slot: t.slot,
                cooldown_ms: t.cooldown_ms,
                conditions: t.conditions.into_iter().map(Into::into).collect(),
                priority: t.priority,
            }
        }
    }

    // ── Per-profile type ──────────────────────────────────────────────────────

    /// Combat vs. out-of-combat clicky profile mode.
    ///
    /// The caller passes this to [`ClickyProfile::items_for_mode`] to filter
    /// items appropriate for the current fight state.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ProfileMode {
        /// Character is currently in combat.
        Combat,
        /// Character is out of combat (medding, idle, etc.).
        OutOfCombat,
    }

    // ── Top-level TOML file ───────────────────────────────────────────────────

    /// Root structure of a `config/clickies/*.toml` file.
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ClickyFile {
        /// All clicky items declared in this file.
        #[serde(default, rename = "clicky")]
        pub clickies: Vec<ClickyItemToml>,
    }

    // ── Merged runtime profile ────────────────────────────────────────────────

    /// Merged, priority-sorted clicky profile ready for [`super::ClickyManager`].
    ///
    /// Built by [`ClickyConfigLoader`] from `global.toml` + `{character}.toml`.
    #[derive(Debug, Clone, Default)]
    pub struct ClickyProfile {
        /// All enabled items sorted ascending by `priority`.
        pub items: Vec<ClickyItem>,
        /// Raw TOML entries (preserved for TUI display / hot-reload diffing).
        pub raw: Vec<ClickyItemToml>,
    }

    impl ClickyProfile {
        /// Build a profile from an iterator of TOML entries.
        ///
        /// Disabled items are filtered out. Remaining items are sorted by
        /// priority before conversion to [`ClickyItem`].
        pub fn from_toml_items(items: impl IntoIterator<Item = ClickyItemToml>) -> Self {
            let mut raw: Vec<ClickyItemToml> =
                items.into_iter().filter(|c| c.enabled).collect();
            raw.sort_by_key(|c| c.priority);
            let converted: Vec<ClickyItem> = raw.iter().cloned().map(Into::into).collect();
            Self { items: converted, raw }
        }

        /// Filter to items appropriate for `mode` (combat vs. out-of-combat).
        ///
        /// - `combat_only = true` items are only returned in [`ProfileMode::Combat`].
        /// - `ooc_only = true` items are only returned in [`ProfileMode::OutOfCombat`].
        /// - Items with neither flag set are returned in both modes.
        #[must_use]
        pub fn items_for_mode(&self, mode: ProfileMode) -> Vec<&ClickyItem> {
            self.raw
                .iter()
                .zip(self.items.iter())
                .filter(|(raw, _)| match mode {
                    ProfileMode::Combat => !raw.ooc_only,
                    ProfileMode::OutOfCombat => !raw.combat_only,
                })
                .map(|(_, item)| item)
                .collect()
        }
    }

    // ── Loader + hot-reload watcher ───────────────────────────────────────────

    /// Loads [`ClickyProfile`]s from disk and detects config changes.
    ///
    /// # Hot-reload
    ///
    /// Call [`ClickyConfigLoader::reload_if_changed`] periodically (e.g. every
    /// second) from the camp loop. The loader compares the max mtime of all
    /// `.toml` files in the config directory to the value at the last successful
    /// load and re-reads on change.
    pub struct ClickyConfigLoader {
        /// Path to the `config/clickies/` directory.
        dir: PathBuf,
        /// Max mtime recorded at the last [`ClickyConfigLoader::load`] call.
        last_mtime: Option<SystemTime>,
    }

    impl ClickyConfigLoader {
        /// Create a loader pointed at `config/clickies/`.
        #[must_use]
        pub fn new() -> Self {
            Self {
                dir: Path::new("config/clickies").to_path_buf(),
                last_mtime: None,
            }
        }

        /// Create a loader pointed at a custom directory (useful for tests).
        #[must_use]
        pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
            Self {
                dir: dir.into(),
                last_mtime: None,
            }
        }

        /// Load the profile for `character_name`.
        ///
        /// 1. Reads `global.toml` (if present).
        /// 2. Appends `{character_name}.toml` (if present).
        /// 3. Returns merged, priority-sorted [`ClickyProfile`].
        ///
        /// Missing files are silently skipped; parse errors are propagated.
        ///
        /// # Errors
        ///
        /// Returns an error if a file exists but cannot be parsed.
        pub fn load(&mut self, character_name: &str) -> Result<ClickyProfile> {
            let mut items: Vec<ClickyItemToml> = Vec::new();

            // 1. Global baseline
            let global_path = self.dir.join("global.toml");
            if global_path.exists() {
                let file = Self::parse_file(&global_path)?;
                items.extend(file.clickies);
            }

            // 2. Character overlay
            let char_path = self.dir.join(format!("{character_name}.toml"));
            if char_path.exists() {
                let file = Self::parse_file(&char_path)?;
                items.extend(file.clickies);
            }

            // Record mtime snapshot for hot-reload detection.
            self.last_mtime = Self::latest_mtime(&self.dir);

            Ok(ClickyProfile::from_toml_items(items))
        }

        /// Parse a single [`ClickyFile`] from disk.
        fn parse_file(path: &Path) -> Result<ClickyFile> {
            let contents = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read clicky config: {}", path.display()))?;
            let file: ClickyFile = toml::from_str(&contents).with_context(|| {
                format!("Failed to parse clicky config: {}", path.display())
            })?;
            Ok(file)
        }

        /// Returns `true` if any `.toml` file in the config dir has been
        /// modified since the last [`ClickyConfigLoader::load`] call.
        #[must_use]
        pub fn is_changed(&self) -> bool {
            match (Self::latest_mtime(&self.dir), self.last_mtime) {
                (Some(current), Some(last)) => current > last,
                (Some(_), None) => true, // never loaded yet
                _ => false,
            }
        }

        /// Reload the profile for `character_name` if the config dir changed.
        ///
        /// Returns `Some(profile)` when a reload occurred, `None` when nothing
        /// changed.
        ///
        /// # Errors
        ///
        /// Propagates parse errors from [`ClickyConfigLoader::load`].
        pub fn reload_if_changed(
            &mut self,
            character_name: &str,
        ) -> Result<Option<ClickyProfile>> {
            if self.is_changed() {
                Ok(Some(self.load(character_name)?))
            } else {
                Ok(None)
            }
        }

        /// Walk `dir` and return the latest mtime across all `.toml` files.
        fn latest_mtime(dir: &Path) -> Option<SystemTime> {
            let entries = std::fs::read_dir(dir).ok()?;
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path().extension().and_then(|s| s.to_str()) == Some("toml")
                })
                .filter_map(|e| e.metadata().ok()?.modified().ok())
                .max()
        }

        /// Recommended polling interval for hot-reload checks.
        pub const POLL_INTERVAL: Duration = Duration::from_secs(1);
    }

    impl Default for ClickyConfigLoader {
        fn default() -> Self {
            Self::new()
        }
    }

    // ── Config tests ──────────────────────────────────────────────────────────

    #[cfg(test)]
    mod tests {
        use super::*;

        fn write_file(dir: &Path, name: &str, contents: &str) {
            std::fs::write(dir.join(name), contents).unwrap();
        }

        // ── TOML parsing ─────────────────────────────────────────────────────

        #[test]
        fn parse_minimal_toml_item() {
            let toml_str = r#"
[[clicky]]
name = "Heal Potion"
slot = 5
"#;
            let file: ClickyFile = toml::from_str(toml_str).unwrap();
            assert_eq!(file.clickies.len(), 1);
            let item = &file.clickies[0];
            assert_eq!(item.name, "Heal Potion");
            assert_eq!(item.slot, 5);
            assert!(item.enabled); // default = true
            assert_eq!(item.cooldown_ms, 0);
            assert_eq!(item.priority, 0);
            assert!(item.conditions.is_empty());
        }

        #[test]
        fn parse_full_toml_item() {
            let toml_str = r#"
[[clicky]]
name        = "Epic Proc"
slot        = 22
cooldown_ms = 72000
priority    = 1
enabled     = true
note        = "Haste clicky"
combat_only = true

[[clicky.conditions]]
hp_pct_max = 90.0
missing_buffs = ["Swift Like the Wind"]
"#;
            let file: ClickyFile = toml::from_str(toml_str).unwrap();
            assert_eq!(file.clickies.len(), 1);
            let item = &file.clickies[0];
            assert_eq!(item.cooldown_ms, 72_000);
            assert_eq!(item.priority, 1);
            assert_eq!(item.note.as_deref(), Some("Haste clicky"));
            assert!(item.combat_only);
            assert!(!item.ooc_only);
            assert_eq!(item.conditions.len(), 1);
            assert_eq!(item.conditions[0].hp_pct_max, Some(90.0));
            assert_eq!(item.conditions[0].missing_buffs, vec!["Swift Like the Wind"]);
        }

        #[test]
        fn parse_invalid_toml_returns_error() {
            let toml_str = "[[clicky]]\nname = "; // malformed
            let result: std::result::Result<ClickyFile, _> = toml::from_str(toml_str);
            assert!(result.is_err());
        }

        #[test]
        fn missing_fields_use_defaults() {
            // Only required fields provided
            let toml_str = r#"
[[clicky]]
name = "Boots"
slot = 0
"#;
            let file: ClickyFile = toml::from_str(toml_str).unwrap();
            let item = &file.clickies[0];
            assert!(item.enabled);
            assert_eq!(item.cooldown_ms, 0);
            assert_eq!(item.priority, 0);
            assert!(item.conditions.is_empty());
            assert!(item.note.is_none());
            assert!(!item.combat_only);
            assert!(!item.ooc_only);
        }

        // ── ClickConditionToml → ClickCondition conversion ────────────────────

        #[test]
        fn condition_conversion_preserves_fields() {
            let toml_cond = ClickConditionToml {
                hp_pct_max: Some(50.0),
                mana_pct_max: Some(30.0),
                missing_buffs: vec!["Haste".into()],
            };
            let cond: ClickCondition = toml_cond.into();
            assert_eq!(cond.hp_pct_max, Some(50.0));
            assert_eq!(cond.mana_pct_max, Some(30.0));
            assert_eq!(cond.missing_buffs, vec!["Haste"]);
        }

        // ── ClickyItemToml → ClickyItem conversion ────────────────────────────

        #[test]
        fn item_conversion_preserves_fields() {
            let toml_item = ClickyItemToml {
                name: "Boots".into(),
                slot: 3,
                cooldown_ms: 5_000,
                priority: 2,
                enabled: true,
                conditions: vec![],
                note: None,
                combat_only: false,
                ooc_only: false,
            };
            let item: ClickyItem = toml_item.into();
            assert_eq!(item.name, "Boots");
            assert_eq!(item.slot, 3);
            assert_eq!(item.cooldown_ms, 5_000);
            assert_eq!(item.priority, 2);
        }

        // ── ClickyProfile ─────────────────────────────────────────────────────

        #[test]
        fn disabled_item_omitted_from_profile() {
            let items = vec![
                ClickyItemToml {
                    name: "active".into(),
                    slot: 0,
                    cooldown_ms: 0,
                    priority: 0,
                    enabled: true,
                    conditions: vec![],
                    note: None,
                    combat_only: false,
                    ooc_only: false,
                },
                ClickyItemToml {
                    name: "inactive".into(),
                    slot: 1,
                    cooldown_ms: 0,
                    priority: 0,
                    enabled: false,
                    conditions: vec![],
                    note: None,
                    combat_only: false,
                    ooc_only: false,
                },
            ];
            let profile = ClickyProfile::from_toml_items(items);
            assert_eq!(profile.items.len(), 1);
            assert_eq!(profile.items[0].name, "active");
        }

        #[test]
        fn profile_sorted_by_priority() {
            let items = vec![
                ClickyItemToml {
                    name: "low".into(),
                    slot: 0,
                    cooldown_ms: 0,
                    priority: 10,
                    enabled: true,
                    conditions: vec![],
                    note: None,
                    combat_only: false,
                    ooc_only: false,
                },
                ClickyItemToml {
                    name: "high".into(),
                    slot: 1,
                    cooldown_ms: 0,
                    priority: 1,
                    enabled: true,
                    conditions: vec![],
                    note: None,
                    combat_only: false,
                    ooc_only: false,
                },
            ];
            let profile = ClickyProfile::from_toml_items(items);
            assert_eq!(profile.items[0].name, "high");
            assert_eq!(profile.items[1].name, "low");
        }

        #[test]
        fn items_for_mode_combat_excludes_ooc_only() {
            let items = vec![
                ClickyItemToml {
                    name: "combat".into(),
                    slot: 0,
                    cooldown_ms: 0,
                    priority: 0,
                    enabled: true,
                    conditions: vec![],
                    note: None,
                    combat_only: true,
                    ooc_only: false,
                },
                ClickyItemToml {
                    name: "ooc".into(),
                    slot: 1,
                    cooldown_ms: 0,
                    priority: 0,
                    enabled: true,
                    conditions: vec![],
                    note: None,
                    combat_only: false,
                    ooc_only: true,
                },
                ClickyItemToml {
                    name: "both".into(),
                    slot: 2,
                    cooldown_ms: 0,
                    priority: 0,
                    enabled: true,
                    conditions: vec![],
                    note: None,
                    combat_only: false,
                    ooc_only: false,
                },
            ];
            let profile = ClickyProfile::from_toml_items(items);

            let combat_items = profile.items_for_mode(ProfileMode::Combat);
            assert_eq!(combat_items.len(), 2);
            assert!(combat_items.iter().any(|i| i.name == "combat"));
            assert!(combat_items.iter().any(|i| i.name == "both"));
            assert!(!combat_items.iter().any(|i| i.name == "ooc"));

            let ooc_items = profile.items_for_mode(ProfileMode::OutOfCombat);
            assert_eq!(ooc_items.len(), 2);
            assert!(ooc_items.iter().any(|i| i.name == "ooc"));
            assert!(ooc_items.iter().any(|i| i.name == "both"));
            assert!(!ooc_items.iter().any(|i| i.name == "combat"));
        }

        // ── ClickyConfigLoader ────────────────────────────────────────────────

        #[test]
        fn loader_loads_global_only_when_no_char_file() {
            let tmp = tempfile::tempdir().unwrap();
            write_file(
                tmp.path(),
                "global.toml",
                r#"
[[clicky]]
name = "Global Potion"
slot = 5
"#,
            );

            let mut loader = ClickyConfigLoader::with_dir(tmp.path());
            let profile = loader.load("Kira").unwrap();
            assert_eq!(profile.items.len(), 1);
            assert_eq!(profile.items[0].name, "Global Potion");
        }

        #[test]
        fn loader_merges_global_and_char_file() {
            let tmp = tempfile::tempdir().unwrap();
            write_file(
                tmp.path(),
                "global.toml",
                r#"
[[clicky]]
name = "Global"
slot = 0
"#,
            );
            write_file(
                tmp.path(),
                "Kira.toml",
                r#"
[[clicky]]
name = "Char Specific"
slot = 1
priority = 1
"#,
            );

            let mut loader = ClickyConfigLoader::with_dir(tmp.path());
            let profile = loader.load("Kira").unwrap();
            assert_eq!(profile.items.len(), 2);
        }

        #[test]
        fn loader_empty_dir_returns_empty_profile() {
            let tmp = tempfile::tempdir().unwrap();
            let mut loader = ClickyConfigLoader::with_dir(tmp.path());
            let profile = loader.load("Kira").unwrap();
            assert!(profile.items.is_empty());
        }

        #[test]
        fn loader_missing_dir_returns_empty_profile() {
            let tmp = tempfile::tempdir().unwrap();
            let missing = tmp.path().join("no_such_dir");
            let mut loader = ClickyConfigLoader::with_dir(missing);
            let profile = loader.load("Kira").unwrap();
            assert!(profile.items.is_empty());
        }

        #[test]
        fn loader_is_changed_after_write() {
            let tmp = tempfile::tempdir().unwrap();
            write_file(
                tmp.path(),
                "global.toml",
                r#"
[[clicky]]
name = "Initial"
slot = 0
"#,
            );

            let mut loader = ClickyConfigLoader::with_dir(tmp.path());
            loader.load("Kira").unwrap(); // baseline

            // Overwrite with different content; mtime advances
            std::thread::sleep(std::time::Duration::from_millis(10));
            write_file(
                tmp.path(),
                "global.toml",
                r#"
[[clicky]]
name = "Updated"
slot = 0
"#,
            );

            assert!(loader.is_changed());
        }

        #[test]
        fn loader_reload_if_changed_returns_none_when_unchanged() {
            let tmp = tempfile::tempdir().unwrap();
            write_file(
                tmp.path(),
                "global.toml",
                r#"
[[clicky]]
name = "Stable"
slot = 0
"#,
            );

            let mut loader = ClickyConfigLoader::with_dir(tmp.path());
            loader.load("Kira").unwrap();

            // No writes → unchanged
            let result = loader.reload_if_changed("Kira").unwrap();
            assert!(result.is_none());
        }

        #[test]
        fn loader_reload_if_changed_returns_some_after_write() {
            let tmp = tempfile::tempdir().unwrap();
            write_file(
                tmp.path(),
                "global.toml",
                r#"
[[clicky]]
name = "Before"
slot = 0
"#,
            );

            let mut loader = ClickyConfigLoader::with_dir(tmp.path());
            loader.load("Kira").unwrap();

            std::thread::sleep(std::time::Duration::from_millis(10));
            write_file(
                tmp.path(),
                "global.toml",
                r#"
[[clicky]]
name = "After"
slot = 0
"#,
            );

            let result = loader.reload_if_changed("Kira").unwrap();
            assert!(result.is_some());
            let profile = result.unwrap();
            assert_eq!(profile.items[0].name, "After");
        }

        #[test]
        fn parse_hp_pct_max_condition_roundtrip() {
            let toml_str = r#"
[[clicky]]
name = "Emergency Heal"
slot = 5

[[clicky.conditions]]
hp_pct_max = 30.0
"#;
            let file: ClickyFile = toml::from_str(toml_str).unwrap();
            let item = &file.clickies[0];
            assert_eq!(item.conditions[0].hp_pct_max, Some(30.0));
        }

        #[test]
        fn parse_mana_pct_max_condition_roundtrip() {
            let toml_str = r#"
[[clicky]]
name = "Mana Clicky"
slot = 3

[[clicky.conditions]]
mana_pct_max = 20.0
"#;
            let file: ClickyFile = toml::from_str(toml_str).unwrap();
            assert_eq!(file.clickies[0].conditions[0].mana_pct_max, Some(20.0));
        }

        #[test]
        fn profile_items_for_mode_all_returned_when_no_flags() {
            let items = vec![
                ClickyItemToml {
                    name: "neutral".into(),
                    slot: 0,
                    cooldown_ms: 0,
                    priority: 0,
                    enabled: true,
                    conditions: vec![],
                    note: None,
                    combat_only: false,
                    ooc_only: false,
                },
            ];
            let profile = ClickyProfile::from_toml_items(items);
            assert_eq!(profile.items_for_mode(ProfileMode::Combat).len(), 1);
            assert_eq!(profile.items_for_mode(ProfileMode::OutOfCombat).len(), 1);
        }

        #[test]
        fn loader_char_file_only_no_global() {
            let tmp = tempfile::tempdir().unwrap();
            // No global.toml — only character file
            write_file(
                tmp.path(),
                "Warrior01.toml",
                r#"
[[clicky]]
name = "Endurance"
slot = 7
"#,
            );

            let mut loader = ClickyConfigLoader::with_dir(tmp.path());
            let profile = loader.load("Warrior01").unwrap();
            assert_eq!(profile.items.len(), 1);
            assert_eq!(profile.items[0].name, "Endurance");
        }
    }
}
