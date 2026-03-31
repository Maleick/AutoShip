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

/// Read-only snapshot of combat-relevant state, passed to strategy methods each tick.
pub struct CombatContext<'a> {
    pub player: &'a SpawnData,
    pub target: Option<&'a SpawnData>,
    pub nearby_enemies: &'a [SpawnData],
    pub group_members: &'a [GroupMemberState],
    pub config: &'a CombatConfig,
    pub tick: u32,
    pub in_combat: bool,
}

#[derive(Debug, Clone)]
pub struct GroupMemberState {
    pub spawn_id: u32,
    pub hp_pct: f32,
    pub mana_pct: f32,
    pub class_id: u8,
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

    /// Minimum enemy count before switching to AoE rotation.
    fn aoe_threshold(&self) -> u8;

    /// Combat role for this strategy.
    fn role(&self) -> CombatRole;
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
}
