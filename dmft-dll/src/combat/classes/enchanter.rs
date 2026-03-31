use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Enchanter strategy: crowd control, mezzes off-targets, nukes when only one enemy.
///
/// When multiple enemies are present, the enchanter picks the first unmezzed
/// off-target for CC.  "Unmezzed" is approximated by skipping the primary
/// assist target (index 0 in `nearby_enemies`) and choosing the next mob.
/// Future: integrate with `MezQueue` for proper expiry-aware target selection.
pub struct EnchanterStrategy {
    class_id: u8,
}

impl EnchanterStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for EnchanterStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        if ctx.nearby_enemies.len() > 1 {
            // The main-assist target is typically the first enemy in the list.
            // Pick the first off-target that is NOT the current target (avoid
            // re-targeting something the group is already burning down).
            let current_target_id = ctx.target.map(|t| t.spawn_id);
            let off_target = ctx
                .nearby_enemies
                .iter()
                .skip(1) // skip primary assist target
                .find(|s| Some(s.spawn_id) != current_target_id);

            // Fall back to the 2nd mob if all off-targets happen to match current target.
            off_target
                .or_else(|| ctx.nearby_enemies.get(1))
                .map(|s| s.spawn_id)
        } else {
            // Single target — use current target for nuking.
            ctx.target.map(|t| t.spawn_id)
        }
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        // spawn_type: 0 = Player, 1 = NPC. Mez NPCs, nuke players (PvP) or assist target.
        // In group XP, off-targets are NPCs that should be mezzed.
        let is_mez_target = ctx
            .target
            .map(|t| t.spawn_type == 1) // NPC spawn type
            .unwrap_or(false);

        let spells = &ctx.config.spells;
        if spells.is_empty() {
            return None;
        }

        if is_mez_target && ctx.nearby_enemies.len() > 1 {
            // Return highest priority spell (mez should be highest priority in enchanter config).
            spells.iter().max_by_key(|s| s.priority).cloned()
        } else {
            // Return lowest priority spell (nuke is lower priority than mez).
            spells.iter().min_by_key(|s| s.priority).cloned()
        }
    }

    fn should_assist(&self, ctx: &CombatContext) -> bool {
        // Assist only when there is a single enemy (no CC needed).
        ctx.nearby_enemies.len() <= 1
    }

    fn aoe_threshold(&self) -> u8 {
        // Never AoE, use single-target CC instead.
        255
    }

    fn role(&self) -> CombatRole {
        CombatRole::CrowdControl
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use dmft_common::combat::CombatConfig;
    use dmft_common::types::SpawnData;

    fn make_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        enemies: &'a [SpawnData],
        config: &'a CombatConfig,
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: enemies,
            group_members: &[],
            config,
            tick: 0,
            in_combat: true,
        }
    }

    #[test]
    fn enchanter_class_id() {
        let enc = EnchanterStrategy::new(14);
        assert_eq!(enc.class_id(), 14);
    }

    #[test]
    fn enchanter_role_is_cc() {
        let enc = EnchanterStrategy::new(14);
        assert_eq!(enc.role(), CombatRole::CrowdControl);
    }

    #[test]
    fn enchanter_never_aoes() {
        let enc = EnchanterStrategy::new(14);
        assert_eq!(enc.aoe_threshold(), 255);
    }

    #[test]
    fn should_assist_when_single_enemy() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let enemies = vec![SpawnData {
            spawn_id: 1,
            ..SpawnData::default()
        }];
        let ctx = make_ctx(&player, None, &enemies, &config);
        assert!(enc.should_assist(&ctx));
    }

    #[test]
    fn should_not_assist_with_multiple_enemies() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx(&player, None, &enemies, &config);
        assert!(!enc.should_assist(&ctx));
    }

    #[test]
    fn select_target_single_enemy_returns_current_target() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };
        let enemies = vec![target.clone()];
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &enemies, &config);
        assert_eq!(enc.select_target(&ctx), Some(42));
    }

    #[test]
    fn select_target_multiple_enemies_picks_off_target() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData::default();
        let primary = SpawnData {
            spawn_id: 1,
            ..SpawnData::default()
        };
        let off1 = SpawnData {
            spawn_id: 2,
            ..SpawnData::default()
        };
        let off2 = SpawnData {
            spawn_id: 3,
            ..SpawnData::default()
        };
        let enemies = vec![primary.clone(), off1, off2];
        let config = CombatConfig::default();
        // Current target is mob 1, enchanter should pick mob 2 (first off-target)
        let ctx = make_ctx(&player, Some(&primary), &enemies, &config);
        assert_eq!(enc.select_target(&ctx), Some(2));
    }

    #[test]
    fn select_target_multiple_enemies_no_current_target() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData::default();
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                ..SpawnData::default()
            },
        ];
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &enemies, &config);
        // No current target, so first off-target (index 1) won't match current_target_id (None)
        assert_eq!(enc.select_target(&ctx), Some(2));
    }

    #[test]
    fn select_spell_mez_for_npc_with_multiple_enemies() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData {
            mana_current: 5000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_type: 1,
            ..SpawnData::default()
        }; // NPC
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                ..SpawnData::default()
            },
        ];
        let config = CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Mesmerize".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Nuke".into(),
                    min_mana_pct: 10.0,
                    priority: 1,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, Some(&target), &enemies, &config);
        let spell = enc.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Mesmerize"); // highest priority = mez
    }

    #[test]
    fn select_spell_nuke_for_single_target() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData {
            mana_current: 5000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_type: 1,
            ..SpawnData::default()
        };
        let enemies = vec![SpawnData {
            spawn_id: 1,
            ..SpawnData::default()
        }];
        let config = CombatConfig {
            spells: vec![
                dmft_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Mesmerize".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                dmft_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Nuke".into(),
                    min_mana_pct: 10.0,
                    priority: 1,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, Some(&target), &enemies, &config);
        let spell = enc.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Nuke"); // lowest priority = nuke
    }

    #[test]
    fn select_spell_empty_spells_returns_none() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &[], &config);
        assert!(enc.select_spell(&ctx).is_none());
    }
}
