use dmft_common::combat::{CombatRole, SpellEntry};
use dmft_common::types::SpawnData;
use dmft_common::nav::Waypoint;

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Berserker strategy: pure melee DPS with frenzy/rage abilities.
///
/// Berserkers are the highest sustained melee DPS class:
/// - Frenzy (primary attack ability)
/// - Rage/bloodlust buffs
/// - No spells — all disc/ability based
/// - Can throw axes at range
pub struct BerserkerStrategy {
    class_id: u8,
}

impl BerserkerStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    fn nearest_enemy<'a>(&self, player: &SpawnData, enemies: &'a [SpawnData]) -> Option<&'a SpawnData> {
        let player_pos = Waypoint::new(player.x, player.y, player.z);
        enemies.iter().min_by(|a, b| {
            let dist_a = player_pos.distance_2d(&Waypoint::new(a.x, a.y, a.z));
            let dist_b = player_pos.distance_2d(&Waypoint::new(b.x, b.y, b.z));
            dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

impl ClassStrategy for BerserkerStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        if ctx.in_combat {
            ctx.target.map(|t| t.spawn_id)
        } else {
            self.nearest_enemy(ctx.player, ctx.nearby_enemies)
                .map(|s| s.spawn_id)
        }
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        // Berserkers use abilities (modeled as spells with high priority).
        // Frenzy is the bread-and-butter.
        ctx.config
            .spells
            .iter()
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        if let Some(target) = ctx.target {
            tracing::info!(
                target_id = target.spawn_id,
                target_name = %target.name,
                "Berserker engaging — FRENZY!"
            );
        }
    }

    fn on_kill(&mut self, _ctx: &CombatContext) {}

    fn aoe_threshold(&self) -> u8 {
        2 // Berserkers excel at AoE with frenzy
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsMelee
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn berserker_role_is_melee_dps() {
        let ber = BerserkerStrategy::new(16);
        assert!(matches!(ber.role(), CombatRole::DpsMelee));
    }
}
