use textquest_common::combat::{AbilityCandidate, EQExpansion, AbilitySet, CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// HP threshold above which clerics should cancel current heal (duck to
/// interrupt). Prevents wasting mana on a heal when the target is already
/// healthy.
const HEAL_CANCEL_THRESHOLD: f32 = 85.0;

/// HP threshold for emergency heals — anyone below this gets top-priority
/// healing.
const EMERGENCY_HP: f32 = 30.0;

/// HP threshold for moderate heals — below this, cast a standard heal.
const MODERATE_HP: f32 = 65.0;

/// Mana reserve threshold: skip non-essential “primary target” debuffs when
/// mana is too low.
const DEBUFF_RESERVE_MANA: f32 = 80.0;

/// Cleric strategy: healer with resurrection, prioritized heal tiers, buff
/// support.
///
/// Priority order (MQ2-style cascade):
/// 0. CH chain override (when active, cast Complete Heal on chain target)
/// 1. Resurrect dead group members (out of combat)
/// 2. Emergency heal (group member < 30% HP)
/// 3. Cure detrimental effects (poison/disease/curse)
/// 4. Moderate heal (group member < 65% HP)
/// 5. Out-of-combat: group buffs
/// 6. Med (sit for mana regen)
pub struct ClericStrategy {
    class_id: u8,
}

impl ClericStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    /// Build cleric ability sets — maps heal/buff/rez lines to
    /// level-tiered candidates, strongest first.
    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "MainHeal".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Ethereal Light".into(),
                        min_level: 65,
                        spell_id: 5739,
                    },
                    AbilityCandidate {
                        name: "Ethereal Remedy".into(),
                        min_level: 63,
                        spell_id: 5738,
                    },
                    AbilityCandidate {
                        name: "Complete Heal".into(),
                        min_level: 39,
                        spell_id: 13,
                    },
                    AbilityCandidate {
                        name: "Superior Healing".into(),
                        min_level: 34,
                        spell_id: 4950,
                    },
                    AbilityCandidate {
                        name: "Greater Healing".into(),
                        min_level: 20,
                        spell_id: 15,
                    },
                    AbilityCandidate {
                        name: "Healing".into(),
                        min_level: 9,
                        spell_id: 12,
                    },
                    AbilityCandidate {
                        name: "Light Healing".into(),
                        min_level: 5,
                        spell_id: 200,
                    },
                    AbilityCandidate {
                        name: "Minor Healing".into(),
                        min_level: 1,
                        spell_id: 201,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "GroupHeal".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Word of Restoration".into(),
                        min_level: 60,
                        spell_id: 3577,
                    },
                    AbilityCandidate {
                        name: "Word of Replenishment".into(),
                        min_level: 55,
                        spell_id: 2175,
                    },
                    AbilityCandidate {
                        name: "Word of Health".into(),
                        min_level: 30,
                        spell_id: 2176,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Rez".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Reviviscence".into(),
                        min_level: 56,
                        spell_id: 1524,
                    },
                    AbilityCandidate {
                        name: "Resurrection".into(),
                        min_level: 47,
                        spell_id: 391,
                    },
                    AbilityCandidate {
                        name: "Revive".into(),
                        min_level: 12,
                        spell_id: 392,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "HpBuff".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Aegolism".into(),
                        min_level: 60,
                        spell_id: 1447,
                    },
                    AbilityCandidate {
                        name: "Symbol of Naltron".into(),
                        min_level: 57,
                        spell_id: 1448,
                    },
                    AbilityCandidate {
                        name: "Symbol of Marzin".into(),
                        min_level: 44,
                        spell_id: 1449,
                    },
                    AbilityCandidate {
                        name: "Symbol of Pinzarn".into(),
                        min_level: 34,
                        spell_id: 1450,
                    },
                    AbilityCandidate {
                        name: "Courage".into(),
                        min_level: 1,
                        spell_id: 14,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "CureDisease".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Abolish Disease".into(),
                        min_level: 58,
                        spell_id: 2060,
                    },
                    AbilityCandidate {
                        name: "Cure Disease".into(),
                        min_level: 4,
                        spell_id: 2054,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "CurePoison".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Abolish Poison".into(),
                        min_level: 59,
                        spell_id: 2061,
                    },
                    AbilityCandidate {
                        name: "Cure Poison".into(),
                        min_level: 1,
                        spell_id: 2055,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
        ]
    }

    /// Find a dead group member who needs resurrection.
    fn dead_member<'a>(&self, ctx: &CombatContext<'a>) -> Option<&'a str> {
        ctx.group_members
            .iter()
            .find(|m| m.is_dead)
            .map(|m| m.name.as_str())
    }

    /// Find a rez spell in the spell list (by name convention).
    fn find_rez_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        ctx.config
            .spells
            .iter()
            .find(|s| {
                let name = s.name.to_lowercase();
                name.contains("resurrect")
                    || name.contains("reviviscence")
                    || name.contains("renewal")
                    || name.contains("rez")
            })
            .cloned()
    }

    /// Find a buff spell (HP buffs, AC buffs, symbol/aegolism).
    fn find_buff_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        ctx.config
            .spells
            .iter()
            .filter(|s| {
                let name = s.name.to_lowercase();
                name.contains("symbol")
                    || name.contains("aegolism")
                    || name.contains("armor")
                    || name.contains("guard")
                    || name.contains("buff")
            })
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    /// Find an in-combat debuff spell (prevents “everyone healthy” from
    /// falling through to med without applying primary-target debuffs).
    fn find_debuff_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        ctx.config
            .spells
            .iter()
            .filter(|s| is_debuff_spell(s))
            .filter(|s| mana_pct >= DEBUFF_RESERVE_MANA && mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    /// Find a cure spell (remove poison, disease, curse).
    fn find_cure_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        let cures: Vec<&SpellEntry> = ctx
            .config
            .spells
            .iter()
            .filter(|s| is_cure_spell(s))
            .filter(|s| mana_pct >= s.min_mana_pct)
            .collect();
        if strategy::afflicted_member_count(ctx) > 1
            && let Some(group_cure) = cures
                .iter()
                .copied()
                .filter(|s| strategy::is_group_cure_spell(s))
                .max_by_key(|s| s.priority)
        {
            return Some(group_cure.clone());
        }
        cures.into_iter().max_by_key(|s| s.priority).cloned()
    }

    /// Find the highest-priority group member who should receive a
    /// single-target cure.
    fn afflicted_member(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::prioritized_afflicted_member(ctx).map(|(spawn_id, _)| spawn_id)
    }

    fn cure_target(&self, ctx: &CombatContext) -> Option<u32> {
        if strategy::afflicted_member_count(ctx) > 1
            && self
                .find_cure_spell(ctx)
                .is_some_and(|spell| strategy::is_group_cure_spell(&spell))
        {
            return Some(ctx.player.spawn_id);
        }
        self.afflicted_member(ctx)
    }

    /// Check if the cleric should cancel an in-progress heal because the target
    /// has recovered above threshold. Called from the combat FSM during Casting
    /// state.
    pub fn should_cancel_heal(&self, ctx: &CombatContext) -> bool {
        let Some((_, lowest_hp)) = strategy::lowest_hp_member(ctx) else {
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
        // Priority 1: Dead group member for rez — signal to the FSM that we need
        // corpse targeting.
        if !ctx.in_combat && self.dead_member(ctx).is_some() && self.find_rez_spell(ctx).is_some() {
            return None; // don't override target — rez spell selection handles it
        }

        // Priority 2: Afflicted group member for cure
        if let Some(afflicted_id) = self.cure_target(ctx) {
            return Some(afflicted_id);
        }

        // Priority 3: Lowest HP group member for healing
        strategy::lowest_hp_member(ctx).map(|(id, _)| id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Priority 0: CH chain override — when the orchestrator tells us to cast CH,
        // we obey unconditionally. The chain coordinator handles timing.
        if let Some(slot) = ctx.ch_chain_slot {
            if let Some(ch_spell) = ctx.config.spells.iter().find(|s| s.slot == slot) {
                tracing::info!(slot, spell = %ch_spell.name, "Cleric: CH chain — casting");
                return Some(ch_spell.clone());
            }
        }

        // Priority 1: Resurrect dead group members (only out of combat)
        if !ctx.in_combat {
            if self.dead_member(ctx).is_some() {
                if let Some(rez) = self.find_rez_spell(ctx) {
                    if mana_pct >= rez.min_mana_pct {
                        tracing::info!(spell = %rez.name, "Cleric: casting resurrection");
                        return Some(rez);
                    }
                }
            }
        }

        // Priority 2: Emergency heal — highest priority spell.
        // Emergency heal MUST fire before cure: if a group member is at 10% HP
        // with a detrimental, healing them is more urgent than curing the DoT.
        if let Some((_, lowest_hp)) = strategy::lowest_hp_member(ctx) {
            if lowest_hp < EMERGENCY_HP {
                return ctx
                    .config
                    .spells
                    .iter()
                    .filter(|s| !is_rez_spell(s) && !is_buff_spell(s) && !is_cure_spell(s))
                    .filter(|s| mana_pct >= s.min_mana_pct)
                    .max_by_key(|s| s.priority)
                    .cloned();
            }
        }

        // Priority 3: Cure detrimental effects (poison/disease/curse)
        if self.afflicted_member(ctx).is_some() {
            if let Some(cure) = self.find_cure_spell(ctx) {
                tracing::info!(spell = %cure.name, "Cleric: curing detrimental");
                return Some(cure);
            }
        }

        let (_, lowest_hp) = strategy::lowest_hp_member(ctx)?;

        // Priority 4: Moderate heal — lower priority (efficient) spell
        if lowest_hp < MODERATE_HP {
            return ctx
                .config
                .spells
                .iter()
                .filter(|s| !is_rez_spell(s) && !is_buff_spell(s) && !is_cure_spell(s))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .min_by_key(|s| s.priority)
                .cloned();
        }

        // Priority 5: In-combat debuffs (debuff before DPS).
        if ctx.in_combat {
            if let Some(debuff) = self.find_debuff_spell(ctx) {
                tracing::info!(spell = %debuff.name, "Cleric: debuffing before DPS");
                return Some(debuff);
            }
        }

        // Priority 6: Out-of-combat buffs
        if !ctx.in_combat {
            if let Some(buff) = self.find_buff_spell(ctx) {
                return Some(buff);
            }
        }

        // Priority 7: Everyone is healthy, med up.
        None
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

    fn ability_sets(&self) -> Vec<AbilitySet> {
        Self::build_ability_sets()
    }

    fn heal_cancel_threshold(&self) -> Option<f32> {
        Some(HEAL_CANCEL_THRESHOLD)
    }

    fn is_heal_cast(&self, ctx: &CombatContext, spell_slot: u8, spell_id: i32) -> bool {
        strategy::spell_for_cast(ctx, spell_slot, spell_id).is_some_and(|spell| {
            let name = spell.name.to_ascii_lowercase();
            name.contains("heal")
                || name.contains("remedy")
                || name.contains("restoration")
                || name.contains("replenishment")
                || name.starts_with("word of ")
        })
    }
}

/// Check if a spell entry is a resurrection spell.
fn is_rez_spell(s: &SpellEntry) -> bool {
    let name = s.name.to_lowercase();
    name.contains("resurrect")
        || name.contains("reviviscence")
        || name.contains("renewal")
        || name.contains("rez")
}

/// Check if a spell entry is a cure spell (remove poison, disease, curse).
fn is_cure_spell(s: &SpellEntry) -> bool {
    let name = s.name.to_lowercase();
    strategy::is_standard_cure_spell(s) || name.contains("abolish") || name.contains("radiant cure")
}

/// Check if a spell entry is a direct-healing spell.
fn is_heal_spell(s: &SpellEntry) -> bool {
    let name = s.name.to_ascii_lowercase();
    name.contains("heal")
        || name.contains("remedy")
        || name.contains("restoration")
        || name.contains("replenishment")
        || name.starts_with("word of ")
}

/// Check if a spell entry is a buff spell.
fn is_buff_spell(s: &SpellEntry) -> bool {
    let name = s.name.to_lowercase();
    name.contains("symbol")
        || name.contains("aegolism")
        || name.contains("armor")
        || name.contains("guard")
        || name.contains("buff")
}

/// Cleric-specific “primary-target debuff” heuristic.
///
/// The cleric spell list contains both healing support and damage/debuff
/// utility; when the group is stable we apply debuffs rather than med.
fn is_debuff_spell(s: &SpellEntry) -> bool {
    let name = s.name.to_lowercase();
    name.contains("mark of") || name.contains("kings") || name.contains("reproach")
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::combat::strategy::GroupMemberState;

    fn make_member(spawn_id: u32, hp_pct: f32, is_dead: bool) -> GroupMemberState {
        GroupMemberState {
            spawn_id,
            hp_pct,
            mana_pct: 100.0,
            class_id: 1,
            is_dead,
            name: format!("Player{spawn_id}"),
            has_detrimental: false,
        }
    }

    fn heal_spell(name: &str, priority: u8) -> SpellEntry {
        SpellEntry {
            slot: priority,
            spell_id: priority as i32 * 100,
            name: name.to_string(),
            min_mana_pct: 10.0,
            priority,
            is_aoe: false,
        }
    }

    /// Build a test context. Returns the config (caller must keep alive) and a
    /// context that borrows it. Uses a raw pointer cast to tie lifetimes — safe
    /// because tests are single-threaded and the config outlives the context.
    fn make_config(spells: &[SpellEntry]) -> textquest_common::combat::CombatConfig {
        textquest_common::combat::CombatConfig {
            spells: spells.to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn cleric_role_is_healer() {
        let cleric = ClericStrategy::new(2);
        assert_eq!(cleric.role(), CombatRole::Healer);
    }

    #[test]
    fn cleric_does_not_assist() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData::default();
        let config = textquest_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };
        assert!(!cleric.should_assist(&ctx));
    }

    #[test]
    fn emergency_heal_uses_highest_priority() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 20.0, false)]; // below EMERGENCY_HP
        let spells = vec![heal_spell("Minor Heal", 1), heal_spell("Complete Heal", 10)];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Complete Heal"); // highest priority
    }

    #[test]
    fn moderate_heal_uses_efficient_spell() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 50.0, false)]; // between EMERGENCY and MODERATE
        let spells = vec![heal_spell("Minor Heal", 1), heal_spell("Complete Heal", 10)];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Minor Heal"); // lowest priority = most efficient
    }

    #[test]
    fn everyone_healthy_returns_none() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 95.0, false)]; // above MODERATE_HP
        let spells = vec![heal_spell("Minor Heal", 1)];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        assert!(cleric.select_spell(&ctx).is_none());
    }

    #[test]
    fn cancel_heal_when_group_recovered() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData::default();
        let members = vec![make_member(10, 90.0, false)]; // above HEAL_CANCEL_THRESHOLD
        let config = textquest_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };
        assert!(cleric.should_cancel_heal(&ctx));
    }

    #[test]
    fn dont_cancel_heal_when_group_still_hurt() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData::default();
        let members = vec![make_member(10, 40.0, false)]; // below threshold
        let config = textquest_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };
        assert!(!cleric.should_cancel_heal(&ctx));
    }

    #[test]
    fn rez_spell_out_of_combat() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![
            make_member(10, 0.0, true),   // dead
            make_member(11, 80.0, false), // alive
        ];
        let spells = vec![
            heal_spell("Minor Heal", 1),
            SpellEntry {
                slot: 8,
                spell_id: 391,
                name: "Resurrection".to_string(),
                min_mana_pct: 10.0,
                priority: 20,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Resurrection");
    }

    #[test]
    fn no_rez_during_combat() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![
            make_member(10, 0.0, true),   // dead
            make_member(11, 25.0, false), // alive but critical
        ];
        let spells = vec![
            heal_spell("Minor Heal", 1),
            heal_spell("Complete Heal", 10),
            SpellEntry {
                slot: 8,
                spell_id: 391,
                name: "Resurrection".to_string(),
                min_mana_pct: 10.0,
                priority: 20,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        // Should heal the living, not rez the dead during combat
        assert_ne!(spell.name, "Resurrection");
        assert_eq!(spell.name, "Complete Heal"); // emergency heal
    }

    #[test]
    fn buff_when_idle_and_healthy() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 95.0, false)]; // healthy
        let spells = vec![
            heal_spell("Minor Heal", 1),
            SpellEntry {
                slot: 5,
                spell_id: 500,
                name: "Symbol of Naltron".to_string(),
                min_mana_pct: 10.0,
                priority: 5,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Symbol of Naltron");
    }

    #[test]
    fn no_buff_during_combat() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 95.0, false)]; // healthy
        let spells = vec![
            heal_spell("Minor Heal", 1),
            SpellEntry {
                slot: 5,
                spell_id: 500,
                name: "Symbol of Naltron".to_string(),
                min_mana_pct: 10.0,
                priority: 5,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        // In combat with everyone healthy — should return None (med)
        assert!(cleric.select_spell(&ctx).is_none());
    }

    #[test]
    fn lowest_hp_excludes_dead_members() {
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![
            make_member(10, 0.0, true),   // dead — should be skipped
            make_member(11, 50.0, false), // alive, hurt
        ];
        let config = textquest_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let (id, hp) = strategy::lowest_hp_member(&ctx).unwrap();
        assert_eq!(id, 11);
        assert!((hp - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cure_takes_priority_over_moderate_heal() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let mut afflicted = make_member(10, 55.0, false);
        afflicted.has_detrimental = true;
        let members = vec![afflicted];
        let spells = vec![
            heal_spell("Minor Heal", 1),
            SpellEntry {
                slot: 6,
                spell_id: 600,
                name: "Cure Disease".to_string(),
                min_mana_pct: 10.0,
                priority: 15,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Cure Disease");
    }

    #[test]
    fn cure_target_prefers_lowest_hp_afflicted_member() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData::default();
        let config = textquest_common::combat::CombatConfig::default();
        let target = textquest_common::types::SpawnData {
            spawn_id: 99,
            ..Default::default()
        };
        let mut sturdy = make_member(10, 80.0, false);
        sturdy.has_detrimental = true;
        let mut fragile = make_member(11, 45.0, false);
        fragile.has_detrimental = true;
        let members = vec![sturdy, fragile];
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        assert_eq!(cleric.select_target(&ctx), Some(11));
    }

    #[test]
    fn group_cure_targets_self_for_multiple_afflicted_members() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            spawn_id: 7,
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let mut first = make_member(10, 80.0, false);
        first.has_detrimental = true;
        let mut second = make_member(11, 60.0, false);
        second.has_detrimental = true;
        let members = vec![first, second];
        let spells = vec![
            SpellEntry {
                slot: 6,
                spell_id: 600,
                name: "Cure Disease".to_string(),
                min_mana_pct: 10.0,
                priority: 15,
                is_aoe: false,
            },
            SpellEntry {
                slot: 7,
                spell_id: 601,
                name: "Radiant Cure".to_string(),
                min_mana_pct: 10.0,
                priority: 5,
                is_aoe: true,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        assert_eq!(cleric.select_spell(&ctx).unwrap().name, "Radiant Cure");
        assert_eq!(cleric.select_target(&ctx), Some(7));
    }

    #[test]
    fn emergency_heal_overrides_cure() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        // Member is afflicted BUT also critically low HP — emergency heal wins
        let mut critical = make_member(10, 15.0, false);
        critical.has_detrimental = true;
        let members = vec![critical];
        let spells = vec![
            heal_spell("Complete Heal", 10),
            SpellEntry {
                slot: 6,
                spell_id: 600,
                name: "Cure Disease".to_string(),
                min_mana_pct: 10.0,
                priority: 15,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        // Emergency heal fires before cure — keeping the member alive is
        // more urgent than removing the damage source when HP is critical.
        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Complete Heal");
    }

    #[test]
    fn ch_chain_override_takes_absolute_priority() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 20.0, false)]; // emergency HP
        let spells = vec![
            heal_spell("Minor Heal", 1),
            heal_spell("Complete Heal", 10),
            SpellEntry {
                slot: 8,
                spell_id: 12,
                name: "Complete Heal".to_string(),
                min_mana_pct: 50.0,
                priority: 100,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: Some(8), // Chain says: cast gem 8
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.slot, 8); // Must obey chain, not regular priority
    }

    #[test]
    fn no_cure_when_no_affliction() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            mana_current: 100,
            mana_max: 100,
            ..Default::default()
        };
        let members = vec![make_member(10, 55.0, false)]; // hurt but not afflicted
        let spells = vec![
            heal_spell("Minor Heal", 1),
            SpellEntry {
                slot: 6,
                spell_id: 600,
                name: "Cure Disease".to_string(),
                min_mana_pct: 10.0,
                priority: 15,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Minor Heal"); // moderate heal, not cure
    }

    // ── AbilitySet tests ────────────────────────────────────────

    fn cleric_known_spells() -> Vec<textquest_common::combat::KnownAbility> {
        ClericStrategy::build_ability_sets()
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
    fn cleric_has_ability_sets() {
        let cleric = ClericStrategy::new(2);
        let sets = cleric.ability_sets();
        assert!(!sets.is_empty());
        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"MainHeal"));
        assert!(names.contains(&"GroupHeal"));
        assert!(names.contains(&"Rez"));
        assert!(names.contains(&"HpBuff"));
        assert!(names.contains(&"CureDisease"));
        assert!(names.contains(&"CurePoison"));
    }

    #[test]
    fn cleric_heal_resolution_at_65() {
        let sets = ClericStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &cleric_known_spells(), 65);
        let heal = resolved.get("MainHeal").expect("should resolve MainHeal");
        assert_eq!(heal.ability_name, "Ethereal Light");
    }

    #[test]
    fn cleric_heal_resolution_at_39() {
        let sets = ClericStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &cleric_known_spells(), 39);
        let heal = resolved.get("MainHeal").expect("should resolve MainHeal");
        assert_eq!(heal.ability_name, "Complete Heal");
    }

    #[test]
    fn cleric_heal_resolution_at_10() {
        let sets = ClericStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &cleric_known_spells(), 10);
        let heal = resolved.get("MainHeal").expect("should resolve MainHeal");
        assert_eq!(heal.ability_name, "Healing");
        // No group heal at level 10
        assert!(!resolved.contains_key("GroupHeal"));
    }

    #[test]
    fn cleric_rez_resolution_scales() {
        let sets = ClericStrategy::build_ability_sets();
        let known = cleric_known_spells();
        let r15 = textquest_common::combat::resolve_abilities(&sets, &known, 15);
        assert_eq!(r15.get("Rez").unwrap().ability_name, "Revive");
        let r50 = textquest_common::combat::resolve_abilities(&sets, &known, 50);
        assert_eq!(r50.get("Rez").unwrap().ability_name, "Resurrection");
        let r60 = textquest_common::combat::resolve_abilities(&sets, &known, 60);
        assert_eq!(r60.get("Rez").unwrap().ability_name, "Reviviscence");
    }

    #[test]
    fn cleric_no_abilities_at_level_0() {
        let sets = ClericStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &cleric_known_spells(), 0);
        assert!(resolved.is_empty());
    }

    #[test]
    fn cleric_buff_resolution() {
        let sets = ClericStrategy::build_ability_sets();
        let known = cleric_known_spells();
        let r1 = textquest_common::combat::resolve_abilities(&sets, &known, 1);
        assert_eq!(r1.get("HpBuff").unwrap().ability_name, "Courage");
        let r60 = textquest_common::combat::resolve_abilities(&sets, &known, 60);
        assert_eq!(r60.get("HpBuff").unwrap().ability_name, "Aegolism");
    }

    #[test]
    fn high_level_cleric_uses_debuff_before_dps_when_group_is_stable() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            level: 65,
            mana_current: 92,
            mana_max: 100,
            ..Default::default()
        };
        let target = textquest_common::types::SpawnData {
            spawn_id: 99,
            hp_current: 9000,
            hp_max: 10000,
            ..Default::default()
        };
        let members = vec![make_member(10, 96.0, false), make_member(11, 91.0, false)];
        let spells = vec![
            SpellEntry {
                slot: 1,
                spell_id: 700,
                name: "Mark of Kings".to_string(),
                min_mana_pct: 80.0,
                priority: 8,
                is_aoe: false,
            },
            SpellEntry {
                slot: 2,
                spell_id: 701,
                name: "Reproach".to_string(),
                min_mana_pct: 90.0,
                priority: 5,
                is_aoe: false,
            },
        ];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let spell = cleric.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Mark of Kings");
    }

    #[test]
    fn high_level_cleric_skips_dps_when_below_reserve_mana() {
        let cleric = ClericStrategy::new(2);
        let player = textquest_common::types::SpawnData {
            level: 65,
            mana_current: 74,
            mana_max: 100,
            ..Default::default()
        };
        let target = textquest_common::types::SpawnData {
            spawn_id: 99,
            hp_current: 9000,
            hp_max: 10000,
            ..Default::default()
        };
        let members = vec![make_member(10, 96.0, false)];
        let spells = vec![SpellEntry {
            slot: 1,
            spell_id: 701,
            name: "Reproach".to_string(),
            min_mana_pct: 20.0,
            priority: 5,
            is_aoe: false,
        }];
        let config = make_config(&spells);
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        assert!(cleric.select_spell(&ctx).is_none());
    }
}
