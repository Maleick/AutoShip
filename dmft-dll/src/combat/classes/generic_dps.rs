use dmft_common::combat::{CombatConfig, CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Generic DPS strategy: works for any DPS class (melee or ranged).
/// Assists main assist, uses highest priority spell that mana allows.
pub struct GenericDpsStrategy {
    class_id: u8,
    role: CombatRole,
    aoe_threshold: u8,
}

impl GenericDpsStrategy {
    pub fn new(class_id: u8, config: &CombatConfig) -> Self {
        Self {
            class_id,
            role: config.role,
            aoe_threshold: config.aoe_threshold,
        }
    }
}

impl ClassStrategy for GenericDpsStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // Pass through current target (assist target).
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Highest priority spell from config where mana is sufficient.
        ctx.config
            .spells
            .iter()
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn aoe_threshold(&self) -> u8 {
        self.aoe_threshold
    }

    fn role(&self) -> CombatRole {
        self.role
    }
}
