//! BandolierManager — MQ2Bandolier parity.
//!
//! Provides full weapon-loadout lifecycle: save / activate / delete plus a
//! per-class TOML rule engine that fires swap commands each game tick.
//!
//! The manager wraps EQ's own `/bandolier` slash-command interface; it does
//! NOT write to disk directly.  EQ's ini layer handles persistence.

use std::{collections::HashMap, path::Path, sync::Mutex};

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum ticks between consecutive bandolier swaps (~2 s at 30 ticks/s).
/// EQ enforces a ~1-second hardware cooldown; we add an extra buffer.
const MIN_SWAP_INTERVAL_TICKS: u64 = 60;

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// One item entry within a named bandolier set.
#[derive(Debug, Clone, PartialEq)]
pub struct BandolierItem {
    pub name: String,
    pub item_id: u32,
    pub slot: u8,
}

/// A named weapon / shield loadout captured from EQ's ini.
#[derive(Debug, Clone, PartialEq)]
pub struct BandolierSet {
    pub name: String,
    pub items: Vec<BandolierItem>,
}

// ---------------------------------------------------------------------------
// Rule engine types
// ---------------------------------------------------------------------------

/// Condition that gates a bandolier auto-swap rule.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BandolierCondition {
    /// Tank in the group has aggro on the current target.
    TankHasAggro,
    /// Player is currently in combat.
    InCombat,
    /// Player is out of combat (downtime / med / etc.).
    OutOfCombat,
    /// Distance to current target exceeds this many EQ units.
    DistanceAbove(f32),
    /// Distance to current target is below this many EQ units.
    DistanceBelow(f32),
}

/// One conditional swap rule: when `condition` is satisfied, activate
/// `bandolier_set`.
#[derive(Debug, Clone, Deserialize)]
pub struct BandolierRule {
    pub condition: BandolierCondition,
    pub bandolier_set: String,
}

/// Root TOML structure for per-class bandolier rule files.
///
/// ```toml
/// [[rules]]
/// condition = "tank_has_aggro"
/// bandolier_set = "Shield Set"
///
/// [[rules]]
/// condition = "out_of_combat"
/// bandolier_set = "2H Set"
/// ```
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BandolierRuleConfig {
    #[serde(default)]
    pub rules: Vec<BandolierRule>,
}

/// Lightweight snapshot of game state for rule evaluation.
/// Decoupled from `CombatContext` so the manager is unit-testable without
/// pulling in live EQ memory structures.
#[derive(Debug, Default, Clone)]
pub struct BandolierContext {
    pub in_combat: bool,
    pub tank_has_aggro: bool,
    pub target_distance: Option<f32>,
}

// ---------------------------------------------------------------------------
// BandolierManager
// ---------------------------------------------------------------------------

/// Full bandolier state machine for one EQ client.
pub struct BandolierManager {
    /// Set names read from the character ini (for display / rule validation).
    known_sets: Vec<String>,
    /// Name of the currently active bandolier set as last observed.
    active_set: Option<String>,
    /// Tick at which the last swap was queued (cooldown gate).
    last_swap_tick: u64,
    /// Auto-swap rules loaded from a per-class TOML file.
    rules: Vec<BandolierRule>,
}

impl BandolierManager {
    pub const fn new() -> Self {
        Self {
            known_sets: Vec::new(),
            active_set: None,
            last_swap_tick: 0,
            rules: Vec::new(),
        }
    }

    // ------------------------------------------------------------------
    // Configuration
    // ------------------------------------------------------------------

    /// Load known set names from an EQ character ini file.
    pub fn load_from_ini(&mut self, path: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read ini: {e}"))?;
        self.known_sets = parse_eq_bandolier_ini(&content);
        Ok(())
    }

    /// Load auto-swap rules from a TOML string.
    pub fn load_rules_toml(&mut self, toml_str: &str) -> Result<(), String> {
        let cfg: BandolierRuleConfig = toml::from_str(toml_str)
            .map_err(|e| format!("failed to parse bandolier rules: {e}"))?;
        self.rules = cfg.rules;
        Ok(())
    }

    /// Update the tracked active set.  Called by the game loop when a
    /// `/bandolier activate` command is observed in the command stream.
    pub fn set_active(&mut self, name: Option<String>) {
        self.active_set = name;
    }

    /// Return the currently active set name.
    pub fn active_set(&self) -> Option<&str> {
        self.active_set.as_deref()
    }

    /// Return the known set names loaded from the ini.
    pub fn known_sets(&self) -> &[String] {
        &self.known_sets
    }

    // ------------------------------------------------------------------
    // Command generators
    // ------------------------------------------------------------------

    /// EQ command to save the current loadout as `set_name`.
    pub fn save_command(set_name: &str) -> String {
        format!(r#"/bandolier save "{set_name}""#)
    }

    /// EQ command to delete `set_name`.
    pub fn delete_command(set_name: &str) -> String {
        format!(r#"/bandolier delete "{set_name}""#)
    }

    /// EQ command to activate `set_name`, subject to the cooldown gate.
    ///
    /// Returns `None` when:
    /// - `set_name` is already the active set, or
    /// - fewer than `MIN_SWAP_INTERVAL_TICKS` have elapsed since the last swap.
    pub fn activate_command(&mut self, set_name: &str, current_tick: u64) -> Option<String> {
        if self.active_set.as_deref() == Some(set_name) {
            return None;
        }
        if current_tick.saturating_sub(self.last_swap_tick) < MIN_SWAP_INTERVAL_TICKS {
            return None;
        }
        self.last_swap_tick = current_tick;
        Some(format!(r#"/bandolier activate "{set_name}""#))
    }

    // ------------------------------------------------------------------
    // Rule engine
    // ------------------------------------------------------------------

    /// Evaluate all rules against `ctx` and return the target set name when a
    /// swap is warranted, or `None`.
    ///
    /// Rules are evaluated in order; the first matching rule wins.  The cooldown
    /// gate is checked before returning a set name so callers can always pass
    /// the result directly to `queue_slash_command`.
    pub fn evaluate_rules(&mut self, ctx: &BandolierContext, current_tick: u64) -> Option<String> {
        for rule in &self.rules {
            if self.condition_matches(&rule.condition, ctx) {
                let target = rule.bandolier_set.clone();
                // Skip if already active.
                if self.active_set.as_deref() == Some(&target) {
                    return None;
                }
                // Respect cooldown.
                if current_tick.saturating_sub(self.last_swap_tick) < MIN_SWAP_INTERVAL_TICKS {
                    return None;
                }
                self.last_swap_tick = current_tick;
                return Some(target);
            }
        }
        None
    }

    fn condition_matches(&self, cond: &BandolierCondition, ctx: &BandolierContext) -> bool {
        match cond {
            BandolierCondition::TankHasAggro => ctx.tank_has_aggro,
            BandolierCondition::InCombat => ctx.in_combat,
            BandolierCondition::OutOfCombat => !ctx.in_combat,
            BandolierCondition::DistanceAbove(d) => {
                ctx.target_distance.is_some_and(|td| td > *d)
            }
            BandolierCondition::DistanceBelow(d) => {
                ctx.target_distance.is_some_and(|td| td < *d)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Global instance
// ---------------------------------------------------------------------------

pub(crate) static BANDOLIER_MANAGER: Mutex<BandolierManager> =
    Mutex::new(BandolierManager::new());

// ---------------------------------------------------------------------------
// EQ ini parser
// ---------------------------------------------------------------------------

/// Parse the `[Bandolier]` section from an EQ character ini file.
///
/// Returns the named sets in slot order (0 .. Size-1).  Missing slots are
/// silently dropped.
///
/// EQ format:
/// ```ini
/// [Bandolier]
/// Size=3
/// BandolierItem0Name=Shield Set
/// BandolierItem0Item0=Iron Shield:1234:22
/// BandolierItem1Name=2H Set
/// BandolierItem1Item0=Great Sword:9012:13
/// BandolierItem2Name=Bow Set
/// BandolierItem2Item0=Longbow:3456:11
/// ```
pub fn parse_eq_bandolier_ini(ini_text: &str) -> Vec<String> {
    let mut in_section = false;
    let mut size = 0usize;
    let mut names: HashMap<usize, String> = HashMap::new();

    for raw_line in ini_text.lines() {
        let line = raw_line.trim();

        // Section header detection.
        if line.starts_with('[') {
            in_section = line.eq_ignore_ascii_case("[Bandolier]");
            continue;
        }

        if !in_section || line.is_empty() || line.starts_with(';') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let key_lower = key.to_ascii_lowercase();

        if key_lower == "size" {
            size = value.parse().unwrap_or(0);
        } else if key_lower.starts_with("bandolieritem") && key_lower.ends_with("name") {
            // Extract slot index from "BandolierItem<N>Name".
            let middle =
                &key_lower["bandolieritem".len()..key_lower.len() - "name".len()];
            if let Ok(idx) = middle.parse::<usize>() {
                names.insert(idx, value.to_string());
            }
        }
    }

    (0..size).filter_map(|i| names.remove(&i)).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Command generators
    // -----------------------------------------------------------------------

    #[test]
    fn save_command_format() {
        assert_eq!(
            BandolierManager::save_command("Shield Set"),
            r#"/bandolier save "Shield Set""#
        );
    }

    #[test]
    fn delete_command_format() {
        assert_eq!(
            BandolierManager::delete_command("2H Set"),
            r#"/bandolier delete "2H Set""#
        );
    }

    // -----------------------------------------------------------------------
    // activate_command — cooldown gate + same-set skip
    // -----------------------------------------------------------------------

    #[test]
    fn activate_command_skips_when_already_active() {
        let mut mgr = BandolierManager::new();
        mgr.set_active(Some("Shield Set".to_string()));
        assert_eq!(mgr.activate_command("Shield Set", 100), None);
    }

    #[test]
    fn activate_command_blocked_within_cooldown() {
        let mut mgr = BandolierManager::new();
        mgr.set_active(Some("2H Set".to_string()));
        // First swap succeeds.
        let cmd = mgr.activate_command("Shield Set", 100);
        assert!(cmd.is_some());
        // Second swap within cooldown window is blocked.
        let cmd2 = mgr.activate_command("2H Set", 100 + MIN_SWAP_INTERVAL_TICKS - 1);
        assert_eq!(cmd2, None);
    }

    #[test]
    fn activate_command_succeeds_after_cooldown() {
        let mut mgr = BandolierManager::new();
        mgr.set_active(Some("2H Set".to_string()));
        let _first = mgr.activate_command("Shield Set", 100);
        // Simulate game_loop observing the /bandolier activate command.
        mgr.set_active(Some("Shield Set".to_string()));
        let cmd = mgr.activate_command("2H Set", 100 + MIN_SWAP_INTERVAL_TICKS);
        assert_eq!(cmd, Some(r#"/bandolier activate "2H Set""#.to_string()));
    }

    #[test]
    fn activate_command_returns_correct_format() {
        let mut mgr = BandolierManager::new();
        let cmd = mgr.activate_command("Heal Set", 0);
        assert_eq!(cmd, Some(r#"/bandolier activate "Heal Set""#.to_string()));
    }

    // -----------------------------------------------------------------------
    // Rule engine
    // -----------------------------------------------------------------------

    fn make_mgr_with_rules(toml: &str) -> BandolierManager {
        let mut mgr = BandolierManager::new();
        mgr.load_rules_toml(toml).expect("valid TOML");
        mgr
    }

    #[test]
    fn rule_engine_fires_on_tank_aggro() {
        let mut mgr = make_mgr_with_rules(
            r#"
            [[rules]]
            condition = "tank_has_aggro"
            bandolier_set = "Shield Set"

            [[rules]]
            condition = "out_of_combat"
            bandolier_set = "2H Set"
            "#,
        );
        let ctx = BandolierContext {
            in_combat: true,
            tank_has_aggro: true,
            target_distance: None,
        };
        let result = mgr.evaluate_rules(&ctx, 0);
        assert_eq!(result, Some("Shield Set".to_string()));
    }

    #[test]
    fn rule_engine_first_rule_wins() {
        // Both in_combat and out_of_combat can't both be true, but distance
        // rules can compete with state rules.  Test that first match wins.
        let mut mgr = make_mgr_with_rules(
            r#"
            [[rules]]
            condition = "in_combat"
            bandolier_set = "Melee Set"

            [[rules]]
            condition = "in_combat"
            bandolier_set = "Other Set"
            "#,
        );
        let ctx = BandolierContext {
            in_combat: true,
            tank_has_aggro: false,
            target_distance: None,
        };
        let result = mgr.evaluate_rules(&ctx, 0);
        assert_eq!(result, Some("Melee Set".to_string()));
    }

    #[test]
    fn rule_engine_no_match_returns_none() {
        let mut mgr = make_mgr_with_rules(
            r#"
            [[rules]]
            condition = "tank_has_aggro"
            bandolier_set = "Shield Set"
            "#,
        );
        let ctx = BandolierContext {
            in_combat: false,
            tank_has_aggro: false,
            target_distance: None,
        };
        assert_eq!(mgr.evaluate_rules(&ctx, 0), None);
    }

    #[test]
    fn rule_engine_skips_already_active_set() {
        let mut mgr = make_mgr_with_rules(
            r#"
            [[rules]]
            condition = "out_of_combat"
            bandolier_set = "2H Set"
            "#,
        );
        mgr.set_active(Some("2H Set".to_string()));
        let ctx = BandolierContext {
            in_combat: false,
            tank_has_aggro: false,
            target_distance: None,
        };
        assert_eq!(mgr.evaluate_rules(&ctx, 0), None);
    }

    #[test]
    fn rule_engine_distance_above_triggers() {
        let mut mgr = make_mgr_with_rules(
            r#"
            [[rules]]
            condition = { distance_above = 30.0 }
            bandolier_set = "Bow Set"
            "#,
        );
        let ctx = BandolierContext {
            in_combat: true,
            tank_has_aggro: false,
            target_distance: Some(35.0),
        };
        assert_eq!(mgr.evaluate_rules(&ctx, 0), Some("Bow Set".to_string()));
    }

    // -----------------------------------------------------------------------
    // INI parser
    // -----------------------------------------------------------------------

    const SAMPLE_INI: &str = r#"
[General]
Foo=Bar

[Bandolier]
Size=3
BandolierItem0Name=Shield Set
BandolierItem0Item0=Iron Shield:1234:22
BandolierItem1Name=2H Set
BandolierItem1Item0=Great Sword:9012:13
BandolierItem2Name=Bow Set
BandolierItem2Item0=Longbow:3456:11

[AnotherSection]
X=1
"#;

    #[test]
    fn ini_parser_standard_format() {
        let sets = parse_eq_bandolier_ini(SAMPLE_INI);
        assert_eq!(sets, vec!["Shield Set", "2H Set", "Bow Set"]);
    }

    #[test]
    fn ini_parser_tolerates_blank_lines_and_comments() {
        let ini = r#"
[Bandolier]
; This is a comment
Size=2

BandolierItem0Name=Melee
BandolierItem1Name=Bow
"#;
        let sets = parse_eq_bandolier_ini(ini);
        assert_eq!(sets, vec!["Melee", "Bow"]);
    }

    #[test]
    fn ini_parser_case_insensitive_section_header() {
        let ini = r#"
[BANDOLIER]
Size=1
BandolierItem0Name=Only Set
"#;
        let sets = parse_eq_bandolier_ini(ini);
        assert_eq!(sets, vec!["Only Set"]);
    }

    #[test]
    fn ini_parser_no_bandolier_section_returns_empty() {
        let ini = "[General]\nFoo=Bar\n";
        assert_eq!(parse_eq_bandolier_ini(ini), Vec::<String>::new());
    }

    #[test]
    fn ini_parser_size_zero_returns_empty() {
        let ini = "[Bandolier]\nSize=0\nBandolierItem0Name=Orphan\n";
        assert_eq!(parse_eq_bandolier_ini(ini), Vec::<String>::new());
    }
}
