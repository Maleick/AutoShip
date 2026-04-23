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
        if let Some(max_hp) = self.hp_pct_max
            && snapshot.hp_pct > max_hp
        {
            return false;
        }
        if let Some(max_mana) = self.mana_pct_max
            && snapshot.mana_pct > max_mana
        {
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
    pub fn cooldown_remaining_ms(&self, character_id: u32, item: &ClickyItem, now: Instant) -> u64 {
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
            .filter(|item| self.is_ready(character_id, item, now) && item.conditions_met(snapshot))
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
                    self.is_ready(character_id, item, now) && item.conditions_met(snapshot)
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
        let mut mgr = ClickyManager::new(vec![simple_item("Boots", 0, 0)]).with_history_cap(3);
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
        let mut mgr =
            ClickyManager::new(vec![simple_item("ItemA", 0, 0), simple_item("ItemB", 1, 0)]);
        let t0 = Instant::now();
        let stub = StubExecutor::default();

        mgr.tick(
            t0,
            &[(1, healthy_snapshot()), (2, healthy_snapshot())],
            &stub,
        );

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
        assert!(
            stub.calls.borrow().is_empty(),
            "external use should enforce cooldown"
        );
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
