use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// HP threshold above which clerics should cancel current heal (duck to interrupt).
/// Prevents wasting mana on a heal when the target is already healthy.
const HEAL_CANCEL_THRESHOLD: f32 = 85.0;

/// Cleric strategy: healer, targets lowest HP group member, prioritizes heals by urgency.
/// Cancels heals (duck) when target HP recovers above threshold during cast.
pub struct ClericStrategy {
    class_id: u8,
}

impl ClericStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    /// Find the group member with the lowest HP percentage.
    fn lowest_hp_member(&self, ctx: &CombatContext) -> Option<(u32, f32)> {
        ctx.group_members
            .iter()
            .filter(|m| m.hp_pct > 0.0) // exclude dead members
            .min_by(|a, b| {
                a.hp_pct
                    .partial_cmp(&b.hp_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|m| (m.spawn_id, m.hp_pct))
    }

    /// Check if the cleric should cancel an in-progress heal because the target
    /// has recovered above threshold. Called from the combat FSM during Casting state.
    pub fn should_cancel_heal(&self, ctx: &CombatContext) -> bool {
        let Some((_, lowest_hp)) = self.lowest_hp_member(ctx) else {
            return true; // no one to heal, cancel
        };
        lowest_hp >= HEAL_CANCEL_THRESHOLD
    }
}

impl ClassStrategy for ClericStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // Target the group member with lowest HP for heal targeting.
        self.lowest_hp_member(ctx).map(|(id, _)| id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let (_, lowest_hp) = self.lowest_hp_member(ctx)?;

        if lowest_hp < 50.0 {
            // Emergency: return highest priority heal spell.
            ctx.config.spells.iter().max_by_key(|s| s.priority).cloned()
        } else if lowest_hp < 80.0 {
            // Moderate: return lower priority heal spell.
            ctx.config.spells.iter().min_by_key(|s| s.priority).cloned()
        } else {
            // Everyone is healthy, med up.
            None
        }
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        false
    }

    fn aoe_threshold(&self) -> u8 {
        // Never AoE.
        255
    }

    fn role(&self) -> CombatRole {
        CombatRole::Healer
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::combat::strategy::GroupMemberState;
    use dmft_common::combat::CombatConfig;
    use dmft_common::types::SpawnData;

    fn make_ctx<'a>(
        player: &'a SpawnData,
        config: &'a CombatConfig,
        group: &'a [GroupMemberState],
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target: None,
            nearby_enemies: &[],
            group_members: group,
            config,
            tick: 0,
            in_combat: true,
        }
    }

    #[test]
    fn cleric_class_id() {
        let cleric = ClericStrategy::new(2);
        assert_eq!(cleric.class_id(), 2);
    }

    #[test]
    fn cleric_role_is_healer() {
        let cleric = ClericStrategy::new(2);
        assert_eq!(cleric.role(), CombatRole::Healer);
    }

    #[test]
    fn cleric_never_aoes() {
        let cleric = ClericStrategy::new(2);
        assert_eq!(cleric.aoe_threshold(), 255);
    }

    #[test]
    fn cleric_does_not_assist() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, &config, &[]);
        assert!(!cleric.should_assist(&ctx));
    }

    #[test]
    fn lowest_hp_member_returns_lowest() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let group = vec![
            GroupMemberState {
                spawn_id: 1,
                hp_pct: 80.0,
                mana_pct: 100.0,
                class_id: 1,
            },
            GroupMemberState {
                spawn_id: 2,
                hp_pct: 30.0,
                mana_pct: 100.0,
                class_id: 3,
            },
            GroupMemberState {
                spawn_id: 3,
                hp_pct: 60.0,
                mana_pct: 100.0,
                class_id: 5,
            },
        ];
        let ctx = make_ctx(&player, &config, &group);
        let (id, hp) = cleric.lowest_hp_member(&ctx).unwrap();
        assert_eq!(id, 2);
        assert!((hp - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn lowest_hp_member_excludes_dead() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let group = vec![
            GroupMemberState {
                spawn_id: 1,
                hp_pct: 0.0,
                mana_pct: 100.0,
                class_id: 1,
            }, // dead
            GroupMemberState {
                spawn_id: 2,
                hp_pct: 50.0,
                mana_pct: 100.0,
                class_id: 3,
            },
        ];
        let ctx = make_ctx(&player, &config, &group);
        let (id, _) = cleric.lowest_hp_member(&ctx).unwrap();
        assert_eq!(id, 2);
    }

    #[test]
    fn lowest_hp_member_all_dead_returns_none() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let group = vec![
            GroupMemberState {
                spawn_id: 1,
                hp_pct: 0.0,
                mana_pct: 0.0,
                class_id: 1,
            },
            GroupMemberState {
                spawn_id: 2,
                hp_pct: 0.0,
                mana_pct: 0.0,
                class_id: 3,
            },
        ];
        let ctx = make_ctx(&player, &config, &group);
        assert!(cleric.lowest_hp_member(&ctx).is_none());
    }

    #[test]
    fn lowest_hp_member_empty_group() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, &config, &[]);
        assert!(cleric.lowest_hp_member(&ctx).is_none());
    }

    #[test]
    fn should_cancel_heal_when_all_healthy() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let group = vec![
            GroupMemberState {
                spawn_id: 1,
                hp_pct: 90.0,
                mana_pct: 100.0,
                class_id: 1,
            },
            GroupMemberState {
                spawn_id: 2,
                hp_pct: 95.0,
                mana_pct: 100.0,
                class_id: 3,
            },
        ];
        let ctx = make_ctx(&player, &config, &group);
        assert!(cleric.should_cancel_heal(&ctx));
    }

    #[test]
    fn should_not_cancel_heal_when_someone_injured() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let group = vec![GroupMemberState {
            spawn_id: 1,
            hp_pct: 40.0,
            mana_pct: 100.0,
            class_id: 1,
        }];
        let ctx = make_ctx(&player, &config, &group);
        assert!(!cleric.should_cancel_heal(&ctx));
    }

    #[test]
    fn should_cancel_heal_when_no_group() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, &config, &[]);
        assert!(cleric.should_cancel_heal(&ctx));
    }

    #[test]
    fn select_target_returns_lowest_hp_member_id() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let group = vec![
            GroupMemberState {
                spawn_id: 10,
                hp_pct: 80.0,
                mana_pct: 100.0,
                class_id: 1,
            },
            GroupMemberState {
                spawn_id: 20,
                hp_pct: 25.0,
                mana_pct: 100.0,
                class_id: 3,
            },
        ];
        let ctx = make_ctx(&player, &config, &group);
        assert_eq!(cleric.select_target(&ctx), Some(20));
    }

    #[test]
    fn select_spell_emergency_heal_below_50() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Big Heal".into(),
                    min_mana_pct: 0.0,
                    priority: 10,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Small Heal".into(),
                    min_mana_pct: 0.0,
                    priority: 1,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let group = vec![GroupMemberState {
            spawn_id: 1,
            hp_pct: 30.0,
            mana_pct: 100.0,
            class_id: 1,
        }];
        let ctx = make_ctx(&player, &config, &group);
        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Big Heal"); // highest priority
    }

    #[test]
    fn select_spell_moderate_heal_between_50_and_80() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Big Heal".into(),
                    min_mana_pct: 0.0,
                    priority: 10,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Small Heal".into(),
                    min_mana_pct: 0.0,
                    priority: 1,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let group = vec![GroupMemberState {
            spawn_id: 1,
            hp_pct: 65.0,
            mana_pct: 100.0,
            class_id: 1,
        }];
        let ctx = make_ctx(&player, &config, &group);
        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Small Heal"); // lowest priority
    }

    #[test]
    fn select_spell_none_when_everyone_healthy() {
        let cleric = ClericStrategy::new(2);
        let player = SpawnData::default();
        let config = CombatConfig {
            spells: vec![dmft_common::combat::SpellEntry {
                slot: 1,
                spell_id: 100,
                name: "Heal".into(),
                min_mana_pct: 0.0,
                priority: 10,
                is_aoe: false,
            }],
            ..CombatConfig::default()
        };
        let group = vec![GroupMemberState {
            spawn_id: 1,
            hp_pct: 95.0,
            mana_pct: 100.0,
            class_id: 1,
        }];
        let ctx = make_ctx(&player, &config, &group);
        assert!(cleric.select_spell(&ctx).is_none());
    }
}
