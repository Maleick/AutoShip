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
