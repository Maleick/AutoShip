//! Loot window automation — FSM-driven corpse looting after kills.
//!
//! Replaces the timer-based loot phase placeholder with actual
//! corpse targeting, approach, loot window, and item pickup.
//! All interactions use slash commands via the IPC→DLL→InterpretCmd pipeline.

use std::collections::HashSet;

use crate::camp::personality::PersonalityProfile;

/// Loot rules controlling what happens to items.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LootRules {
    /// Items to always pick up and keep.
    #[serde(default)]
    pub keep_items: HashSet<String>,
    /// Items to pick up for selling to vendors.
    #[serde(default)]
    pub sell_items: HashSet<String>,
    /// Items to pick up and immediately destroy (junk clogging inventory).
    #[serde(default)]
    pub destroy_items: HashSet<String>,
    /// If true, loot everything not in `destroy_items`. Overrides keep/sell lists.
    #[serde(default = "default_true")]
    pub loot_all: bool,
    /// If true, auto-split coin with group.
    #[serde(default = "default_true")]
    pub auto_split: bool,
}

fn default_true() -> bool {
    true
}

impl Default for LootRules {
    fn default() -> Self {
        Self {
            keep_items: HashSet::new(),
            sell_items: HashSet::new(),
            destroy_items: HashSet::new(),
            loot_all: true,
            auto_split: true,
        }
    }
}

/// Configuration for the loot system.
#[derive(Debug, Clone)]
pub struct LootConfig {
    /// Loot rules for item filtering.
    pub rules: LootRules,
    /// Base ticks to wait between individual item pickups (humanization).
    pub item_pickup_delay: u64,
    /// Base ticks to wait after targeting corpse before approaching.
    pub target_delay: u64,
    /// Base ticks to wait for approach before opening loot window.
    pub approach_delay: u64,
    /// Base ticks to wait after opening loot window before picking items.
    pub loot_open_delay: u64,
    /// Base ticks to wait after looting before closing / moving to next corpse.
    pub close_delay: u64,
    /// Hide looted corpses after finishing.
    pub hide_looted_corpses: bool,
}

impl Default for LootConfig {
    fn default() -> Self {
        Self {
            rules: LootRules::default(),
            item_pickup_delay: 2,
            target_delay: 1,
            approach_delay: 3,
            loot_open_delay: 2,
            close_delay: 1,
            hide_looted_corpses: true,
        }
    }
}

/// A corpse to loot, tracked by spawn ID and mob name.
#[derive(Debug, Clone)]
pub struct CorpseEntry {
    pub spawn_id: u32,
    pub mob_name: String,
}

/// Phases of the loot FSM for a single corpse cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LootPhase {
    /// Target the corpse via /target.
    TargetCorpse,
    /// Move within loot range of the corpse.
    ApproachCorpse { entered_tick: u64 },
    /// Open the loot window via /loot.
    OpenLoot { entered_tick: u64 },
    /// Pick up items from the loot window.
    LootItems { entered_tick: u64 },
    /// Close loot window and clean up.
    CloseLoot { entered_tick: u64 },
    /// Move to the next corpse in the queue.
    NextCorpse,
    /// All corpses looted.
    Done,
}

/// The loot FSM — drives the corpse-by-corpse loot cycle.
pub struct LootCycle {
    pub config: LootConfig,
    pub phase: LootPhase,
    pub corpse_queue: Vec<CorpseEntry>,
    pub current_corpse_idx: usize,
}

impl LootCycle {
    pub fn new(config: LootConfig, corpses: Vec<CorpseEntry>) -> Self {
        let phase = if corpses.is_empty() {
            LootPhase::Done
        } else {
            LootPhase::TargetCorpse
        };
        Self {
            config,
            phase,
            corpse_queue: corpses,
            current_corpse_idx: 0,
        }
    }

    /// Is the loot cycle complete?
    pub fn is_done(&self) -> bool {
        self.phase == LootPhase::Done
    }

    /// Get the current corpse being looted, if any.
    fn current_corpse(&self) -> Option<&CorpseEntry> {
        self.corpse_queue.get(self.current_corpse_idx)
    }

    /// Advance the loot FSM by one tick. Returns `(pid, command)` pairs
    /// for the designated looter. Uses personality for delay humanization.
    pub fn tick(
        &mut self,
        looter_pid: u32,
        current_tick: u64,
        personality: &PersonalityProfile,
    ) -> Vec<(u32, String)> {
        let mut commands = Vec::new();

        match self.phase.clone() {
            LootPhase::TargetCorpse => {
                if let Some(corpse) = self.current_corpse() {
                    // Target the corpse: /target <mob_name>'s corpse
                    let target_cmd = format!("/target {}'s corpse", corpse.mob_name);
                    commands.push((looter_pid, target_cmd));
                    self.phase = LootPhase::ApproachCorpse {
                        entered_tick: current_tick,
                    };
                } else {
                    self.phase = LootPhase::Done;
                }
            }

            LootPhase::ApproachCorpse { entered_tick } => {
                let delay = personality.adjust_delay(self.config.approach_delay);
                if current_tick.saturating_sub(entered_tick) >= delay {
                    // Face and move toward corpse
                    commands.push((looter_pid, "/face".into()));
                    // Open loot window
                    commands.push((looter_pid, "/loot".into()));
                    self.phase = LootPhase::OpenLoot {
                        entered_tick: current_tick,
                    };
                }
            }

            LootPhase::OpenLoot { entered_tick } => {
                let delay = personality.adjust_delay(self.config.loot_open_delay);
                if current_tick.saturating_sub(entered_tick) >= delay {
                    // Loot all items
                    if self.config.rules.loot_all {
                        commands.push((looter_pid, "/lootall".into()));
                    }
                    self.phase = LootPhase::LootItems {
                        entered_tick: current_tick,
                    };
                }
            }

            LootPhase::LootItems { entered_tick } => {
                let delay = personality.adjust_delay(self.config.item_pickup_delay);
                if current_tick.saturating_sub(entered_tick) >= delay {
                    // Auto-split coin if enabled
                    if self.config.rules.auto_split {
                        commands.push((looter_pid, "/autosplit".into()));
                    }
                    // Destroy junk items left on cursor
                    if !self.config.rules.destroy_items.is_empty() {
                        commands.push((looter_pid, "/destroy".into()));
                    }
                    self.phase = LootPhase::CloseLoot {
                        entered_tick: current_tick,
                    };
                }
            }

            LootPhase::CloseLoot { entered_tick } => {
                let delay = personality.adjust_delay(self.config.close_delay);
                if current_tick.saturating_sub(entered_tick) >= delay {
                    if self.config.hide_looted_corpses {
                        commands.push((looter_pid, "/hidecorpse looted".into()));
                    }
                    self.phase = LootPhase::NextCorpse;
                }
            }

            LootPhase::NextCorpse => {
                self.current_corpse_idx += 1;
                if self.current_corpse_idx < self.corpse_queue.len() {
                    self.phase = LootPhase::TargetCorpse;
                } else {
                    self.phase = LootPhase::Done;
                }
            }

            LootPhase::Done => {}
        }

        commands
    }
}

/// Determine the item action based on loot rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemAction {
    Keep,
    Sell,
    Destroy,
    Ignore,
}

/// Classify an item based on loot rules.
pub fn classify_item(item_name: &str, rules: &LootRules) -> ItemAction {
    if rules.destroy_items.contains(item_name) {
        return ItemAction::Destroy;
    }
    if rules.keep_items.contains(item_name) {
        return ItemAction::Keep;
    }
    if rules.sell_items.contains(item_name) {
        return ItemAction::Sell;
    }
    if rules.loot_all {
        return ItemAction::Keep;
    }
    ItemAction::Ignore
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> LootConfig {
        LootConfig {
            rules: LootRules {
                keep_items: ["Fine Steel Dagger".into(), "Bone Chips".into()]
                    .into_iter()
                    .collect(),
                sell_items: ["Cracked Staff".into(), "Rusty Dagger".into()]
                    .into_iter()
                    .collect(),
                destroy_items: ["Spider Legs".into()].into_iter().collect(),
                loot_all: false,
                auto_split: true,
            },
            item_pickup_delay: 2,
            target_delay: 1,
            approach_delay: 3,
            loot_open_delay: 2,
            close_delay: 1,
            hide_looted_corpses: true,
        }
    }

    fn test_corpses() -> Vec<CorpseEntry> {
        vec![
            CorpseEntry {
                spawn_id: 1001,
                mob_name: "an orc pawn".into(),
            },
            CorpseEntry {
                spawn_id: 1002,
                mob_name: "an orc centurion".into(),
            },
        ]
    }

    fn neutral_personality() -> PersonalityProfile {
        // Create a personality with reaction_speed = 1.0 for predictable tests
        let mut p = PersonalityProfile::generate("TestLooter");
        p.reaction_speed = 1.0;
        p
    }

    #[test]
    fn test_empty_corpse_queue_is_done() {
        let cycle = LootCycle::new(LootConfig::default(), Vec::new());
        assert!(cycle.is_done());
        assert_eq!(cycle.phase, LootPhase::Done);
    }

    #[test]
    fn test_starts_at_target_corpse() {
        let cycle = LootCycle::new(test_config(), test_corpses());
        assert_eq!(cycle.phase, LootPhase::TargetCorpse);
        assert!(!cycle.is_done());
    }

    #[test]
    fn test_target_corpse_issues_target_command() {
        let mut cycle = LootCycle::new(test_config(), test_corpses());
        let personality = neutral_personality();
        let cmds = cycle.tick(104, 1, &personality);

        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].0, 104);
        assert_eq!(cmds[0].1, "/target an orc pawn's corpse");
        assert!(matches!(cycle.phase, LootPhase::ApproachCorpse { .. }));
    }

    #[test]
    fn test_approach_waits_for_delay() {
        let mut cycle = LootCycle::new(test_config(), test_corpses());
        let personality = neutral_personality();

        // TargetCorpse -> ApproachCorpse
        cycle.tick(104, 1, &personality);
        assert!(matches!(cycle.phase, LootPhase::ApproachCorpse { .. }));

        // Approach delay is 3 ticks — should wait
        let cmds = cycle.tick(104, 2, &personality);
        assert!(cmds.is_empty());
        assert!(matches!(cycle.phase, LootPhase::ApproachCorpse { .. }));

        let cmds = cycle.tick(104, 3, &personality);
        assert!(cmds.is_empty());
        assert!(matches!(cycle.phase, LootPhase::ApproachCorpse { .. }));

        // After 3 ticks: should transition
        let cmds = cycle.tick(104, 4, &personality);
        assert!(!cmds.is_empty());
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/face"));
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/loot"));
        assert!(matches!(cycle.phase, LootPhase::OpenLoot { .. }));
    }

    #[test]
    fn test_full_single_corpse_cycle() {
        let config = LootConfig {
            rules: LootRules {
                loot_all: true,
                auto_split: true,
                ..Default::default()
            },
            item_pickup_delay: 1,
            target_delay: 1,
            approach_delay: 1,
            loot_open_delay: 1,
            close_delay: 1,
            hide_looted_corpses: true,
        };
        let corpses = vec![CorpseEntry {
            spawn_id: 1001,
            mob_name: "an orc pawn".into(),
        }];
        let mut cycle = LootCycle::new(config, corpses);
        let personality = neutral_personality();

        // Tick 1: TargetCorpse -> ApproachCorpse
        let cmds = cycle.tick(104, 1, &personality);
        assert_eq!(cmds[0].1, "/target an orc pawn's corpse");
        assert!(matches!(cycle.phase, LootPhase::ApproachCorpse { .. }));

        // Tick 2: ApproachCorpse -> OpenLoot (delay=1)
        let cmds = cycle.tick(104, 2, &personality);
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/loot"));

        // Tick 3: OpenLoot -> LootItems (delay=1)
        let cmds = cycle.tick(104, 3, &personality);
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/lootall"));

        // Tick 4: LootItems -> CloseLoot (delay=1)
        let cmds = cycle.tick(104, 4, &personality);
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/autosplit"));

        // Tick 5: CloseLoot -> NextCorpse (delay=1)
        let cmds = cycle.tick(104, 5, &personality);
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/hidecorpse looted"));
        assert_eq!(cycle.phase, LootPhase::NextCorpse);

        // Tick 6: NextCorpse -> Done (only 1 corpse)
        let cmds = cycle.tick(104, 6, &personality);
        assert!(cmds.is_empty());
        assert!(cycle.is_done());
    }

    #[test]
    fn test_multi_corpse_cycle() {
        let config = LootConfig {
            item_pickup_delay: 1,
            target_delay: 1,
            approach_delay: 1,
            loot_open_delay: 1,
            close_delay: 1,
            ..LootConfig::default()
        };
        let mut cycle = LootCycle::new(config, test_corpses());
        let personality = neutral_personality();
        let mut tick = 0;

        // Process all corpses
        while !cycle.is_done() {
            tick += 1;
            cycle.tick(104, tick, &personality);
            assert!(tick < 100, "Loot cycle should not loop forever");
        }

        // Should have processed both corpses
        assert_eq!(cycle.current_corpse_idx, 2);
        assert!(cycle.is_done());
    }

    #[test]
    fn test_second_corpse_targets_correctly() {
        let config = LootConfig {
            item_pickup_delay: 1,
            target_delay: 1,
            approach_delay: 1,
            loot_open_delay: 1,
            close_delay: 1,
            ..LootConfig::default()
        };
        let mut cycle = LootCycle::new(config, test_corpses());
        let personality = neutral_personality();

        // Fast-forward through first corpse (6 ticks for single corpse + NextCorpse)
        let mut tick = 0;
        let mut all_cmds = Vec::new();
        while cycle.current_corpse_idx == 0 || cycle.phase == LootPhase::NextCorpse {
            tick += 1;
            let cmds = cycle.tick(104, tick, &personality);
            all_cmds.extend(cmds);
            if cycle.is_done() {
                break;
            }
        }

        // Now we should be targeting the second corpse
        let cmds = cycle.tick(104, tick + 1, &personality);
        // The second corpse target command
        assert!(
            all_cmds
                .iter()
                .chain(cmds.iter())
                .any(|(_, cmd)| cmd.contains("an orc centurion")),
            "Should target the second corpse"
        );
    }

    #[test]
    fn test_personality_affects_delays() {
        let config = LootConfig {
            approach_delay: 4,
            ..LootConfig::default()
        };
        let corpses = vec![CorpseEntry {
            spawn_id: 1001,
            mob_name: "an orc pawn".into(),
        }];
        let mut cycle = LootCycle::new(config, corpses);

        // Fast personality (0.7x) = approach delay of round(4 * 0.7) = 3
        let mut fast_personality = neutral_personality();
        fast_personality.reaction_speed = 0.7;

        // TargetCorpse -> ApproachCorpse at tick 1
        cycle.tick(104, 1, &fast_personality);

        // At tick 3 (2 elapsed), should still wait
        let cmds = cycle.tick(104, 3, &fast_personality);
        assert!(cmds.is_empty());

        // At tick 4 (3 elapsed = adjusted delay), should transition
        let cmds = cycle.tick(104, 4, &fast_personality);
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_no_autosplit_when_disabled() {
        let config = LootConfig {
            rules: LootRules {
                auto_split: false,
                loot_all: true,
                ..Default::default()
            },
            item_pickup_delay: 1,
            target_delay: 1,
            approach_delay: 1,
            loot_open_delay: 1,
            close_delay: 1,
            hide_looted_corpses: false,
        };
        let corpses = vec![CorpseEntry {
            spawn_id: 1001,
            mob_name: "orc".into(),
        }];
        let mut cycle = LootCycle::new(config, corpses);
        let personality = neutral_personality();

        let mut all_cmds = Vec::new();
        let mut tick = 0;
        while !cycle.is_done() {
            tick += 1;
            all_cmds.extend(cycle.tick(104, tick, &personality));
            assert!(tick < 50);
        }

        assert!(
            !all_cmds.iter().any(|(_, cmd)| cmd == "/autosplit"),
            "Should not autosplit when disabled"
        );
        assert!(
            !all_cmds.iter().any(|(_, cmd)| cmd.contains("hidecorpse")),
            "Should not hide corpses when disabled"
        );
    }

    #[test]
    fn test_no_lootall_when_not_loot_all() {
        let config = LootConfig {
            rules: LootRules {
                loot_all: false,
                ..Default::default()
            },
            item_pickup_delay: 1,
            target_delay: 1,
            approach_delay: 1,
            loot_open_delay: 1,
            close_delay: 1,
            hide_looted_corpses: false,
        };
        let corpses = vec![CorpseEntry {
            spawn_id: 1001,
            mob_name: "orc".into(),
        }];
        let mut cycle = LootCycle::new(config, corpses);
        let personality = neutral_personality();

        let mut all_cmds = Vec::new();
        let mut tick = 0;
        while !cycle.is_done() {
            tick += 1;
            all_cmds.extend(cycle.tick(104, tick, &personality));
            assert!(tick < 50);
        }

        assert!(
            !all_cmds.iter().any(|(_, cmd)| cmd == "/lootall"),
            "Should not /lootall when loot_all is disabled"
        );
    }

    // -- classify_item tests --

    #[test]
    fn test_classify_keep_item() {
        let config = test_config();
        assert_eq!(
            classify_item("Fine Steel Dagger", &config.rules),
            ItemAction::Keep
        );
    }

    #[test]
    fn test_classify_sell_item() {
        let config = test_config();
        assert_eq!(
            classify_item("Cracked Staff", &config.rules),
            ItemAction::Sell
        );
    }

    #[test]
    fn test_classify_destroy_item() {
        let config = test_config();
        assert_eq!(
            classify_item("Spider Legs", &config.rules),
            ItemAction::Destroy
        );
    }

    #[test]
    fn test_classify_unknown_item_loot_all_false() {
        let config = test_config();
        // loot_all is false in test_config
        assert_eq!(
            classify_item("Unknown Widget", &config.rules),
            ItemAction::Ignore
        );
    }

    #[test]
    fn test_classify_unknown_item_loot_all_true() {
        let mut config = test_config();
        config.rules.loot_all = true;
        assert_eq!(
            classify_item("Unknown Widget", &config.rules),
            ItemAction::Keep
        );
    }

    #[test]
    fn test_destroy_takes_priority_over_keep() {
        let rules = LootRules {
            keep_items: ["Spider Legs".into()].into_iter().collect(),
            destroy_items: ["Spider Legs".into()].into_iter().collect(),
            ..Default::default()
        };
        // Destroy should win over keep
        assert_eq!(classify_item("Spider Legs", &rules), ItemAction::Destroy);
    }

    #[test]
    fn test_done_phase_is_noop() {
        let mut cycle = LootCycle::new(LootConfig::default(), Vec::new());
        let personality = neutral_personality();
        let cmds = cycle.tick(104, 1, &personality);
        assert!(cmds.is_empty());
        assert!(cycle.is_done());

        // Ticking again should still be a no-op
        let cmds = cycle.tick(104, 2, &personality);
        assert!(cmds.is_empty());
        assert!(cycle.is_done());
    }
}
