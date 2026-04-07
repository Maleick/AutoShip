use textquest_common::combat::{AbilityCandidate, AbilitySet, CombatRole, SpellEntry};

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

    /// Build enchanter ability sets — CC, haste, slow, nuke lines tiered by level.
    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "Mez".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Bliss".into(),
                        min_level: 65,
                        spell_id: 5520,
                    },
                    AbilityCandidate {
                        name: "Glamour of Kintaz".into(),
                        min_level: 60,
                        spell_id: 3341,
                    },
                    AbilityCandidate {
                        name: "Dazzle".into(),
                        min_level: 44,
                        spell_id: 187,
                    },
                    AbilityCandidate {
                        name: "Mesmerize".into(),
                        min_level: 11,
                        spell_id: 185,
                    },
                ],
            },
            AbilitySet {
                name: "AoEMez".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Wake of Felicity".into(),
                        min_level: 62,
                        spell_id: 5521,
                    },
                    AbilityCandidate {
                        name: "Fascination".into(),
                        min_level: 53,
                        spell_id: 2150,
                    },
                    AbilityCandidate {
                        name: "Mesmerization".into(),
                        min_level: 29,
                        spell_id: 189,
                    },
                ],
            },
            AbilitySet {
                name: "Haste".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Speed of Vallon".into(),
                        min_level: 65,
                        spell_id: 5522,
                    },
                    AbilityCandidate {
                        name: "Wonderous Rapidity".into(),
                        min_level: 49,
                        spell_id: 1693,
                    },
                    AbilityCandidate {
                        name: "Alacrity".into(),
                        min_level: 24,
                        spell_id: 170,
                    },
                    AbilityCandidate {
                        name: "Quickness".into(),
                        min_level: 16,
                        spell_id: 171,
                    },
                ],
            },
            AbilitySet {
                name: "Slow".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Dreary Deeds".into(),
                        min_level: 60,
                        spell_id: 3344,
                    },
                    AbilityCandidate {
                        name: "Tepid Deeds".into(),
                        min_level: 52,
                        spell_id: 2151,
                    },
                    AbilityCandidate {
                        name: "Shiftless Deeds".into(),
                        min_level: 24,
                        spell_id: 191,
                    },
                    AbilityCandidate {
                        name: "Languid Pace".into(),
                        min_level: 4,
                        spell_id: 190,
                    },
                ],
            },
            AbilitySet {
                name: "Nuke".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Dementia".into(),
                        min_level: 60,
                        spell_id: 3343,
                    },
                    AbilityCandidate {
                        name: "Sanity Warp".into(),
                        min_level: 44,
                        spell_id: 1694,
                    },
                    AbilityCandidate {
                        name: "Chaotic Feedback".into(),
                        min_level: 1,
                        spell_id: 186,
                    },
                ],
            },
            AbilitySet {
                name: "Tash".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Wind of Tashani".into(),
                        min_level: 62,
                        spell_id: 5523,
                    },
                    AbilityCandidate {
                        name: "Tashania".into(),
                        min_level: 55,
                        spell_id: 2153,
                    },
                    AbilityCandidate {
                        name: "Tashan".into(),
                        min_level: 1,
                        spell_id: 188,
                    },
                ],
            },
            AbilitySet {
                name: "Charm".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Command of Druzzil".into(),
                        min_level: 65,
                        spell_id: 5524,
                    },
                    AbilityCandidate {
                        name: "Allure".into(),
                        min_level: 52,
                        spell_id: 2152,
                    },
                    AbilityCandidate {
                        name: "Beguile".into(),
                        min_level: 24,
                        spell_id: 192,
                    },
                    AbilityCandidate {
                        name: "Charm".into(),
                        min_level: 11,
                        spell_id: 193,
                    },
                ],
            },
        ]
    }
}

impl ClassStrategy for EnchanterStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        if ctx.nearby_enemies.len() > 1 {
            if let Some(add_target_id) = ctx.extended_targets.and_then(|xt| {
                xt.cc_add_spawn_ids().into_iter().find(|spawn_id| {
                    ctx.nearby_enemies
                        .iter()
                        .any(|enemy| enemy.spawn_id == *spawn_id)
                })
            }) {
                return Some(add_target_id);
            }

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
        let is_mez_target = ctx.target.is_some_and(|t| t.spawn_type == 1);

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

    fn ability_sets(&self) -> Vec<AbilitySet> {
        Self::build_ability_sets()
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::combat::{
        CombatConfig, ExtendedTargetList, ExtendedTargetSlot, XTargetSlotStatus, XTargetType,
    };
    use textquest_common::types::SpawnData;

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
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
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
    fn select_target_prefers_xtarget_cc_add() {
        let enc = EnchanterStrategy::new(14);
        let player = SpawnData::default();
        let primary = SpawnData {
            spawn_id: 1,
            ..SpawnData::default()
        };
        let add = SpawnData {
            spawn_id: 2,
            ..SpawnData::default()
        };
        let other = SpawnData {
            spawn_id: 3,
            ..SpawnData::default()
        };
        let enemies = vec![primary.clone(), other, add.clone()];
        let config = CombatConfig::default();
        let xtargets = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 1,
                    name: "main".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::GroupAssistTarget,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 1,
                    name: "main".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 2,
                    name: "add".into(),
                },
            ],
            auto_add_haters: true,
        };
        let mut ctx = make_ctx(&player, Some(&primary), &enemies, &config);
        ctx.extended_targets = Some(&xtargets);

        assert_eq!(enc.select_target(&ctx), Some(2));
    }

    #[test]
    fn select_target_falls_back_when_xtarget_add_missing_from_nearby_enemies() {
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
        let xtargets = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 99,
                    name: "missing".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::GroupAssistTarget,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 1,
                    name: "main".into(),
                },
            ],
            auto_add_haters: true,
        };
        let mut ctx = make_ctx(&player, Some(&primary), &enemies, &config);
        ctx.extended_targets = Some(&xtargets);

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
                textquest_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Mesmerize".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                textquest_common::combat::SpellEntry {
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
                textquest_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Mesmerize".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                textquest_common::combat::SpellEntry {
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

    // ── AbilitySet tests ──────────────���─────────────────────────

    fn enchanter_known_spells() -> Vec<textquest_common::combat::KnownAbility> {
        EnchanterStrategy::build_ability_sets()
            .iter()
            .flat_map(|s| &s.candidates)
            .map(|c| textquest_common::combat::KnownAbility {
                name: c.name.clone(),
                spell_id: c.spell_id,
                level: c.min_level,
            })
            .collect()
    }

    #[test]
    fn enchanter_has_ability_sets() {
        let enc = EnchanterStrategy::new(14);
        let sets = enc.ability_sets();
        assert!(!sets.is_empty());
        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Mez"));
        assert!(names.contains(&"AoEMez"));
        assert!(names.contains(&"Haste"));
        assert!(names.contains(&"Slow"));
        assert!(names.contains(&"Nuke"));
        assert!(names.contains(&"Tash"));
        assert!(names.contains(&"Charm"));
    }

    #[test]
    fn enchanter_mez_resolution_at_65() {
        let sets = EnchanterStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &enchanter_known_spells(), 65);
        let mez = resolved.get("Mez").expect("should resolve Mez");
        assert_eq!(mez.ability_name, "Bliss");
    }

    #[test]
    fn enchanter_mez_resolution_at_45() {
        let sets = EnchanterStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &enchanter_known_spells(), 45);
        let mez = resolved.get("Mez").expect("should resolve Mez");
        assert_eq!(mez.ability_name, "Dazzle");
    }

    #[test]
    fn enchanter_mez_resolution_at_11() {
        let sets = EnchanterStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &enchanter_known_spells(), 11);
        let mez = resolved.get("Mez").expect("should resolve Mez");
        assert_eq!(mez.ability_name, "Mesmerize");
    }

    #[test]
    fn enchanter_haste_scales_with_level() {
        let sets = EnchanterStrategy::build_ability_sets();
        let known = enchanter_known_spells();
        let r20 = textquest_common::combat::resolve_abilities(&sets, &known, 20);
        assert_eq!(r20.get("Haste").unwrap().ability_name, "Quickness");
        let r50 = textquest_common::combat::resolve_abilities(&sets, &known, 50);
        assert_eq!(r50.get("Haste").unwrap().ability_name, "Wonderous Rapidity");
        let r65 = textquest_common::combat::resolve_abilities(&sets, &known, 65);
        assert_eq!(r65.get("Haste").unwrap().ability_name, "Speed of Vallon");
    }

    #[test]
    fn enchanter_slow_scales_with_level() {
        let sets = EnchanterStrategy::build_ability_sets();
        let known = enchanter_known_spells();
        let r5 = textquest_common::combat::resolve_abilities(&sets, &known, 5);
        assert_eq!(r5.get("Slow").unwrap().ability_name, "Languid Pace");
        let r60 = textquest_common::combat::resolve_abilities(&sets, &known, 60);
        assert_eq!(r60.get("Slow").unwrap().ability_name, "Dreary Deeds");
    }

    #[test]
    fn enchanter_no_mez_below_level_11() {
        let sets = EnchanterStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &enchanter_known_spells(), 10);
        assert!(!resolved.contains_key("Mez"));
    }

    #[test]
    fn enchanter_all_lines_resolve_at_65() {
        let sets = EnchanterStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &enchanter_known_spells(), 65);
        assert_eq!(
            resolved.len(),
            7,
            "All 7 enchanter ability lines should resolve at 65"
        );
    }
}
