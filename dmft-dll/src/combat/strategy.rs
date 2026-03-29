use dmft_common::combat::{CombatConfig, CombatRole, SpellEntry};
use dmft_common::types::SpawnData;

use super::classes::cleric::ClericStrategy;
use super::classes::druid::DruidStrategy;
use super::classes::enchanter::EnchanterStrategy;
use super::classes::generic_dps::GenericDpsStrategy;
use super::classes::magician::MagicianStrategy;
use super::classes::monk::MonkStrategy;
use super::classes::necromancer::NecromancerStrategy;
use super::classes::rogue::RogueStrategy;
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
    fn on_engage(&mut self, ctx: &CombatContext);

    /// Called when a mob dies.
    fn on_kill(&mut self, ctx: &CombatContext);

    /// Minimum enemy count before switching to AoE rotation.
    fn aoe_threshold(&self) -> u8;

    /// Combat role for this strategy.
    fn role(&self) -> CombatRole;
}

/// Factory function — creates the right strategy for a given class.
pub fn build_strategy(class_id: u8, config: &CombatConfig) -> Box<dyn ClassStrategy> {
    match class_id {
        1 => Box::new(WarriorStrategy::new(class_id)),       // Warrior
        2 => Box::new(ClericStrategy::new(class_id)),        // Cleric
        5 => Box::new(WizardStrategy::new(class_id)),        // Wizard
        6 => Box::new(DruidStrategy::new(class_id)),         // Druid
        7 => Box::new(MonkStrategy::new(class_id)),          // Monk
        9 => Box::new(RogueStrategy::new(class_id)),         // Rogue
        10 => Box::new(ShamanStrategy::new(class_id)),       // Shaman
        11 => Box::new(NecromancerStrategy::new(class_id)),  // Necromancer
        13 => Box::new(MagicianStrategy::new(class_id)),     // Magician
        14 => Box::new(EnchanterStrategy::new(class_id)),    // Enchanter
        _ => Box::new(GenericDpsStrategy::new(class_id, config)),
    }
}
