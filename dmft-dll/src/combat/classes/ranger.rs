use dmft_common::combat::{CombatRole, SpellEntry};
use dmft_common::nav::Waypoint;
use dmft_common::types::SpawnData;

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Ranger strategy: ranged/melee hybrid DPS with tracking and bow pulling.
///
/// Rangers operate in two stances:
/// - **Ranged**: Use bow attacks and DoT spells from distance (default when pulling)
/// - **Melee**: Switch to melee when target is close, use kicks and backstab-style abilities
///
/// Rangers also provide: tracking (find mobs), snare (Snare/Ensnare),
/// and at higher levels, Headshot AA for trivial kills.
pub struct RangerStrategy {
    class_id: u8,
    /// Distance threshold to switch from ranged to melee (in EQ units).
    melee_range: f32,
}

impl RangerStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            melee_range: 30.0, // Switch to melee within 30 units
        }
    }

    /// Calculate distance to target.
    fn distance_to(&self, player: &SpawnData, target: &SpawnData) -> f32 {
        let p = Waypoint::new(player.x, player.y, player.z);
        let t = Waypoint::new(target.x, target.y, target.z);
        p.distance_2d(&t)
    }

    /// Whether we're in melee range of the target.
    fn in_melee_range(&self, ctx: &CombatContext) -> bool {
        ctx.target
            .map(|t| self.distance_to(ctx.player, t) <= self.melee_range)
            .unwrap_or(false)
    }
}

impl ClassStrategy for RangerStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // Assist MA when in combat, otherwise target nearest.
        if ctx.in_combat {
            ctx.target.map(|t| t.spawn_id)
        } else {
            strategy::nearest_enemy(ctx.player, ctx.nearby_enemies).map(|s| s.spawn_id)
        }
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let spells = &ctx.config.spells;
        if spells.is_empty() {
            return None;
        }

        if self.in_melee_range(ctx) {
            // In melee range: prefer melee abilities (lower spell IDs or higher priority)
            spells
                .iter()
                .filter(|s| s.priority >= 5) // High priority = melee abilities
                .max_by_key(|s| s.priority)
                .or_else(|| spells.iter().max_by_key(|s| s.priority))
                .cloned()
        } else {
            // At range: prefer ranged spells (DoTs, snare, bow)
            spells
                .iter()
                .filter(|s| s.priority < 5) // Lower priority = ranged
                .max_by_key(|s| s.priority)
                .or_else(|| spells.iter().max_by_key(|s| s.priority))
                .cloned()
        }
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true // Rangers assist the MA
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        if let Some(target) = ctx.target {
            let dist = self.distance_to(ctx.player, target);
            let stance = if dist <= self.melee_range {
                "melee"
            } else {
                "ranged"
            };
            tracing::info!(
                target_id = target.spawn_id,
                target_name = %target.name,
                distance = format!("{:.0}", dist),
                stance,
                "Ranger engaging target"
            );
        }
    }

    fn aoe_threshold(&self) -> u8 {
        3
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsRanged
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use dmft_common::combat::{CombatConfig, SpellEntry};

    fn make_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        enemies: &'a [SpawnData],
        config: &'a CombatConfig,
        in_combat: bool,
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: enemies,
            group_members: &[],
            config,
            tick: 0,
            in_combat,
            ch_chain_slot: None,
        }
    }

    #[test]
    fn ranger_class_id() {
        let r = RangerStrategy::new(4);
        assert_eq!(r.class_id(), 4);
    }

    #[test]
    fn ranger_role_is_dps() {
        let ranger = RangerStrategy::new(4);
        assert!(matches!(ranger.role(), CombatRole::DpsRanged));
    }

    #[test]
    fn ranger_aoe_threshold() {
        let r = RangerStrategy::new(4);
        assert_eq!(r.aoe_threshold(), 3);
    }

    #[test]
    fn ranger_assists_ma() {
        let ranger = RangerStrategy::new(4);
        let player = SpawnData::default();
        let config = dmft_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
        };
        assert!(ranger.should_assist(&ctx));
    }

    #[test]
    fn select_target_in_combat_returns_assist_target() {
        let r = RangerStrategy::new(4);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 33,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &[], &config, true);
        assert_eq!(r.select_target(&ctx), Some(33));
    }

    #[test]
    fn select_target_out_of_combat_nearest_enemy() {
        let r = RangerStrategy::new(4);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 100.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 15.0,
                ..SpawnData::default()
            },
        ];
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &enemies, &config, false);
        assert_eq!(r.select_target(&ctx), Some(2));
    }

    #[test]
    fn in_melee_range_true_when_close() {
        let r = RangerStrategy::new(4);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 1,
            x: 20.0, // within 30 unit melee range
            y: 0.0,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &[], &config, true);
        assert!(r.in_melee_range(&ctx));
    }

    #[test]
    fn in_melee_range_false_when_far() {
        let r = RangerStrategy::new(4);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 1,
            x: 100.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &[], &config, true);
        assert!(!r.in_melee_range(&ctx));
    }

    #[test]
    fn in_melee_range_false_no_target() {
        let r = RangerStrategy::new(4);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &[], &config, true);
        assert!(!r.in_melee_range(&ctx));
    }

    #[test]
    fn select_spell_melee_range_prefers_high_priority() {
        let r = RangerStrategy::new(4);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 1,
            x: 10.0, // in melee range
            ..SpawnData::default()
        };
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "RangedDoT".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 3,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Kick".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 7,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], &config, true);
        let spell = r.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Kick"); // priority >= 5, melee ability
    }

    #[test]
    fn select_spell_ranged_prefers_low_priority() {
        let r = RangerStrategy::new(4);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 1,
            x: 100.0, // at range
            ..SpawnData::default()
        };
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "Snare".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 4,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Kick".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 7,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], &config, true);
        let spell = r.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Snare"); // priority < 5, ranged ability
    }

    #[test]
    fn select_spell_empty_returns_none() {
        let r = RangerStrategy::new(4);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &[], &config, false);
        assert!(r.select_spell(&ctx).is_none());
    }

    #[test]
    fn nearest_enemy_finds_closest() {
        let player = SpawnData {
            x: 50.0,
            y: 50.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 200.0,
                y: 200.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 55.0,
                y: 55.0,
                ..SpawnData::default()
            },
        ];
        let nearest = strategy::nearest_enemy(&player, &enemies).unwrap();
        assert_eq!(nearest.spawn_id, 2);
    }
}
