use dmft_common::combat::{CombatConfig, CombatRole, SpellEntry};
use dmft_common::types::SpawnData;

use super::classes::bard::BardStrategy;
use super::classes::beastlord::BeastlordStrategy;
use super::classes::berserker::BerserkerStrategy;
use super::classes::cleric::ClericStrategy;
use super::classes::druid::DruidStrategy;
use super::classes::enchanter::EnchanterStrategy;
use super::classes::generic_dps::GenericDpsStrategy;
use super::classes::magician::MagicianStrategy;
use super::classes::monk::MonkStrategy;
use super::classes::necromancer::NecromancerStrategy;
use super::classes::paladin::PaladinStrategy;
use super::classes::ranger::RangerStrategy;
use super::classes::rogue::RogueStrategy;
use super::classes::shadow_knight::ShadowKnightStrategy;
use super::classes::shaman::ShamanStrategy;
use super::classes::warrior::WarriorStrategy;
use super::classes::wizard::WizardStrategy;

/// Read-only snapshot of combat-relevant state, passed to strategy methods each frame.
pub struct CombatContext<'a> {
    pub player: &'a SpawnData,
    pub target: Option<&'a SpawnData>,
    pub nearby_enemies: &'a [SpawnData],
    pub group_members: &'a [GroupMemberState],
    pub config: &'a CombatConfig,
    pub tick: u32,
    pub in_combat: bool,
    /// When a CH chain is active, this is the spell slot the cleric should cast.
    /// The cleric strategy defers its normal priority cascade and casts CH instead
    /// when this is `Some`. Set by the orchestrator when it's this cleric's turn.
    pub ch_chain_slot: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct GroupMemberState {
    pub spawn_id: u32,
    pub hp_pct: f32,
    pub mana_pct: f32,
    pub class_id: u8,
    /// True if this member is dead (corpse present, needs resurrection).
    pub is_dead: bool,
    /// Character name, used for corpse targeting during resurrection.
    pub name: String,
    /// True if this member has a detrimental effect (poison, disease, curse)
    /// that should be cured. Set by the orchestrator from buff window parsing.
    pub has_detrimental: bool,
}

/// The core seam between generic combat framework and per-class logic.
/// Each EQ class implements this trait to define its combat behavior.
pub trait ClassStrategy: Send {
    /// Which EQ class this strategy handles.
    fn class_id(&self) -> u8;

    /// Select the next spell to cast given current context.
    /// Returns None if no spell should be cast (med, wait, etc.)
    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry>;

    /// Select what to target. For DPS: assist target's target.
    /// For healers: lowest HP group member. For tanks: current mob.
    fn select_target(&self, ctx: &CombatContext) -> Option<u32>;

    /// Whether this character should assist the main assist.
    fn should_assist(&self, ctx: &CombatContext) -> bool;

    /// Called when engaging a new target.
    /// Override to log engagement or toggle auto-attack.
    fn on_engage(&mut self, _ctx: &CombatContext) {}

    /// Called after an action completes (spell cast, ability use, song twist).
    /// Override to advance internal state (e.g., bard twist index, auto-attack toggle).
    fn on_action_complete(&mut self, _ctx: &CombatContext) {}

    /// Called when a cast is interrupted before completion (HolyShit preempt,
    /// target lost mid-cast, or external interrupt like "You miss a note").
    /// `gem` is the spell slot that was being cast when the interrupt occurred.
    /// Default is a no-op; bards override this to re-queue the interrupted song.
    fn on_cast_interrupted(&mut self, _ctx: &CombatContext, _gem: u8) {}

    /// Minimum enemy count before switching to `AoE` rotation.
    fn aoe_threshold(&self) -> u8;

    /// Combat role for this strategy.
    fn role(&self) -> CombatRole;
}

// ---------------------------------------------------------------------------
// Shared melee helpers — reused by warrior, rogue, monk, SK, paladin,
// berserker, beastlord to eliminate duplicated code across class strategies.
// ---------------------------------------------------------------------------

/// Find the nearest NPC from the nearby enemies list based on 2D distance to player.
/// Used by tank/pull-capable classes (warrior, berserker, beastlord) for target selection.
#[inline]
pub fn nearest_enemy<'a>(player: &SpawnData, enemies: &'a [SpawnData]) -> Option<&'a SpawnData> {
    let px = player.x;
    let py = player.y;
    enemies.iter().min_by(|a, b| {
        let dist_sq_a = (a.x - px).powi(2) + (a.y - py).powi(2);
        let dist_sq_b = (b.x - px).powi(2) + (b.y - py).powi(2);
        dist_sq_a
            .partial_cmp(&dist_sq_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// Common assist-target selection: return the current target's spawn ID.
/// Used by DPS melee classes that follow the main assist.
#[inline]
pub fn assist_target(ctx: &CombatContext) -> Option<u32> {
    ctx.target.map(|t| t.spawn_id)
}

/// Common `on_engage` for melee classes: log engagement and enable auto-attack.
pub fn melee_on_engage(ctx: &CombatContext, class_label: &str) {
    if let Some(target) = ctx.target {
        tracing::info!(
            target_id = target.spawn_id,
            target_name = %target.name,
            "{class_label} engaging"
        );
    }
    crate::eq::toggle_auto_attack(true);
}

/// Common disengage for melee classes: disable auto-attack.
/// Only call when `!ctx.in_combat` — mid-combat spell completions should NOT disable auto-attack.
pub fn melee_on_disengage() {
    crate::eq::toggle_auto_attack(false);
}

/// Find the group member with the lowest HP percentage (alive only).
/// Used by healer and hybrid classes (cleric, druid, paladin, shaman) for heal targeting.
#[inline]
pub fn lowest_hp_member(ctx: &CombatContext) -> Option<(u32, f32)> {
    ctx.group_members
        .iter()
        .filter(|m| !m.is_dead && m.hp_pct.is_finite() && m.hp_pct > 0.0)
        .min_by(|a, b| {
            a.hp_pct
                .partial_cmp(&b.hp_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|m| (m.spawn_id, m.hp_pct))
}

/// Select the highest-priority spell from config, filtered by current mana.
#[inline]
pub fn best_spell_by_mana(ctx: &CombatContext) -> Option<SpellEntry> {
    let mana_pct = ctx.player.mana_pct();
    ctx.config
        .spells
        .iter()
        .filter(|s| mana_pct >= s.min_mana_pct)
        .max_by_key(|s| s.priority)
        .cloned()
}

/// Factory function -- creates the right strategy for a given class.
pub fn build_strategy(class_id: u8, config: &CombatConfig) -> Box<dyn ClassStrategy> {
    match class_id {
        1 => Box::new(WarriorStrategy::new(class_id)), // Warrior
        2 => Box::new(ClericStrategy::new(class_id)),  // Cleric
        3 => Box::new(PaladinStrategy::new(class_id)), // Paladin
        4 => Box::new(RangerStrategy::new(class_id)),  // Ranger
        5 => Box::new(ShadowKnightStrategy::new(class_id)), // Shadow Knight
        6 => Box::new(DruidStrategy::new(class_id)),   // Druid
        7 => Box::new(MonkStrategy::new(class_id)),    // Monk
        8 => Box::new(BardStrategy::new(class_id)),    // Bard
        9 => Box::new(RogueStrategy::new(class_id)),   // Rogue
        10 => Box::new(ShamanStrategy::new(class_id)), // Shaman
        11 => Box::new(NecromancerStrategy::new(class_id)), // Necromancer
        12 => Box::new(WizardStrategy::new(class_id)), // Wizard
        13 => Box::new(MagicianStrategy::new(class_id)), // Magician
        14 => Box::new(EnchanterStrategy::new(class_id)), // Enchanter
        15 => Box::new(BeastlordStrategy::new(class_id)), // Beastlord
        16 => Box::new(BerserkerStrategy::new(class_id)), // Berserker
        _ => Box::new(GenericDpsStrategy::new(class_id, config)),
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn build_strategy_all_16_classes() {
        let config = CombatConfig::default();
        for id in 1..=16u8 {
            let strategy = build_strategy(id, &config);
            assert_eq!(
                strategy.class_id(),
                id,
                "build_strategy({}) returned class_id {}",
                id,
                strategy.class_id()
            );
            // Just verify role() doesn't panic
            let _role = strategy.role();
        }
    }

    #[test]
    fn build_strategy_unknown_class_uses_generic() {
        let config = CombatConfig::default();
        let strategy = build_strategy(99, &config);
        assert_eq!(strategy.class_id(), 99);
    }

    #[test]
    fn nearest_enemy_returns_closest() {
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 100.0,
                y: 0.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 10.0,
                y: 0.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 3,
                x: 50.0,
                y: 0.0,
                ..SpawnData::default()
            },
        ];
        let closest = nearest_enemy(&player, &enemies).unwrap();
        assert_eq!(closest.spawn_id, 2);
    }

    #[test]
    fn nearest_enemy_empty_list() {
        let player = SpawnData::default();
        assert!(nearest_enemy(&player, &[]).is_none());
    }

    #[test]
    fn assist_target_returns_target_id() {
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
        };
        assert_eq!(assist_target(&ctx), Some(42));
    }

    #[test]
    fn assist_target_none_without_target() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
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
        assert!(assist_target(&ctx).is_none());
    }

    #[test]
    fn best_spell_by_mana_returns_highest_priority() {
        use dmft_common::combat::SpellEntry;
        let mut player = SpawnData::default();
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "Low".into(),
                    slot: 1,
                    spell_id: 100,
                    priority: 1,
                    min_mana_pct: 10.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "High".into(),
                    slot: 2,
                    spell_id: 200,
                    priority: 10,
                    min_mana_pct: 10.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "TooExpensive".into(),
                    slot: 3,
                    spell_id: 300,
                    priority: 100,
                    min_mana_pct: 90.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
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
        let spell = best_spell_by_mana(&ctx).unwrap();
        assert_eq!(spell.name, "High"); // highest priority that we can afford
    }

    #[test]
    fn best_spell_by_mana_none_when_empty() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
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
        assert!(best_spell_by_mana(&ctx).is_none());
    }

    #[test]
    fn best_spell_by_mana_none_when_all_too_expensive() {
        let mut player = SpawnData::default();
        player.mana_current = 100;
        player.mana_max = 10000; // 1% mana
        let config = CombatConfig {
            spells: vec![SpellEntry {
                name: "Nuke".into(),
                slot: 1,
                spell_id: 1,
                priority: 10,
                min_mana_pct: 50.0,
                is_aoe: false,
            }],
            ..CombatConfig::default()
        };
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
        assert!(best_spell_by_mana(&ctx).is_none());
    }

    #[test]
    fn nearest_enemy_diagonal_distances() {
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 70.0,
                y: 70.0, // distance ~99
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 50.0,
                y: 50.0, // distance ~71
                ..SpawnData::default()
            },
        ];
        let closest = nearest_enemy(&player, &enemies).unwrap();
        assert_eq!(closest.spawn_id, 2);
    }

    #[test]
    fn nearest_enemy_single() {
        let player = SpawnData::default();
        let enemies = vec![SpawnData {
            spawn_id: 42,
            x: 10.0,
            ..SpawnData::default()
        }];
        assert_eq!(nearest_enemy(&player, &enemies).unwrap().spawn_id, 42);
    }

    #[test]
    fn build_strategy_boundary_class_ids() {
        let config = CombatConfig::default();
        // Class ID 0 should use generic
        let s = build_strategy(0, &config);
        assert_eq!(s.class_id(), 0);
        // Class ID 255 should use generic
        let s = build_strategy(255, &config);
        assert_eq!(s.class_id(), 255);
    }

    #[test]
    fn build_strategy_roles_correct() {
        use dmft_common::combat::CombatRole;
        let config = CombatConfig::default();

        assert_eq!(build_strategy(1, &config).role(), CombatRole::MainTank); // Warrior
        assert_eq!(build_strategy(2, &config).role(), CombatRole::Healer); // Cleric
        assert_eq!(build_strategy(3, &config).role(), CombatRole::OffTank); // Paladin
        assert_eq!(build_strategy(4, &config).role(), CombatRole::DpsRanged); // Ranger
        assert_eq!(build_strategy(5, &config).role(), CombatRole::OffTank); // SK
        assert_eq!(build_strategy(6, &config).role(), CombatRole::Healer); // Druid
        assert_eq!(build_strategy(7, &config).role(), CombatRole::DpsMelee); // Monk
        assert_eq!(build_strategy(9, &config).role(), CombatRole::DpsMelee); // Rogue
        assert_eq!(build_strategy(12, &config).role(), CombatRole::DpsRanged); // Wizard
        assert_eq!(build_strategy(13, &config).role(), CombatRole::DpsRanged); // Magician
        assert_eq!(build_strategy(16, &config).role(), CombatRole::DpsMelee); // Berserker
    }

    #[test]
    fn group_member_state_debug() {
        let gms = GroupMemberState {
            spawn_id: 1,
            hp_pct: 75.0,
            mana_pct: 50.0,
            class_id: 2,
            is_dead: false,
            name: String::new(),
            has_detrimental: false,
        };
        let debug = format!("{gms:?}");
        assert!(debug.contains("spawn_id: 1"));
    }

    #[test]
    fn lowest_hp_member_excludes_nan_hp() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let members = vec![
            GroupMemberState {
                spawn_id: 1,
                hp_pct: f32::NAN,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: "NanWarrior".into(),
                has_detrimental: false,
            },
            GroupMemberState {
                spawn_id: 2,
                hp_pct: 50.0,
                mana_pct: 100.0,
                class_id: 2,
                is_dead: false,
                name: "Cleric".into(),
                has_detrimental: false,
            },
        ];
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
        };
        let result = lowest_hp_member(&ctx);
        assert_eq!(result, Some((2, 50.0)));
    }
}
