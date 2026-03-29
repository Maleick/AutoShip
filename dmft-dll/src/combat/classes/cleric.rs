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
            .min_by(|a, b| a.hp_pct.partial_cmp(&b.hp_pct).unwrap_or(std::cmp::Ordering::Equal))
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
            ctx.config
                .spells
                .iter()
                .max_by_key(|s| s.priority)
                .cloned()
        } else if lowest_hp < 80.0 {
            // Moderate: return lower priority heal spell.
            ctx.config
                .spells
                .iter()
                .min_by_key(|s| s.priority)
                .cloned()
        } else {
            // Everyone is healthy, med up.
            None
        }
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        false
    }

    fn on_engage(&mut self, _ctx: &CombatContext) {}

    fn on_action_complete(&mut self, _ctx: &CombatContext) {}

    fn aoe_threshold(&self) -> u8 {
        // Never AoE.
        255
    }

    fn role(&self) -> CombatRole {
        CombatRole::Healer
    }
}
