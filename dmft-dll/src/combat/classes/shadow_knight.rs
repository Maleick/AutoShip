use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Shadow Knight strategy: off-tank with lifetap DPS, disease/poison DoTs, snare.
/// EQ class ID: 5
pub struct ShadowKnightStrategy {
    class_id: u8,
}

impl ShadowKnightStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for ShadowKnightStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::assist_target(ctx)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        let hp_pct = ctx.player.hp_pct();

        // Priority 1: Lifetap when HP is low
        if hp_pct < 60.0
            && let Some(tap) = ctx
                .config
                .spells
                .iter()
                .filter(|s| {
                    s.name.contains("Tap")
                        || s.name.contains("tap")
                        || s.name.contains("Leech")
                        || s.name.contains("Drain")
                })
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
        {
            return Some(tap);
        }

        // Priority 2: Snare on fleeing mob
        if let Some(target) = ctx.target
            && target.hp_pct() < 15.0
            && let Some(snare) = ctx
                .config
                .spells
                .iter()
                .filter(|s| s.name.contains("Snare") || s.name.contains("Darkness"))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
        {
            return Some(snare);
        }

        // Priority 3: Disease/poison DoTs and nukes
        ctx.config
            .spells
            .iter()
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        false // SK tanks, doesn't assist
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Shadow Knight");
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        if !ctx.in_combat {
            strategy::melee_on_disengage();
        }
    }

    fn aoe_threshold(&self) -> u8 {
        2
    }

    fn role(&self) -> CombatRole {
        CombatRole::OffTank
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sk_class_id() {
        let sk = ShadowKnightStrategy::new(5);
        assert_eq!(sk.class_id(), 5);
    }

    #[test]
    fn sk_role_is_off_tank() {
        let sk = ShadowKnightStrategy::new(5);
        assert_eq!(sk.role(), CombatRole::OffTank);
    }

    #[test]
    fn sk_does_not_assist() {
        let sk = ShadowKnightStrategy::new(5);
        let config = dmft_common::combat::CombatConfig::default();
        let player = dmft_common::types::SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
        };
        assert!(!sk.should_assist(&ctx));
    }
}
