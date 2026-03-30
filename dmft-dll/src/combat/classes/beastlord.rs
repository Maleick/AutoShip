use dmft_common::combat::{CombatRole, SpellEntry};
use dmft_common::nav::Waypoint;
use dmft_common::types::SpawnData;

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Beastlord strategy: pet class with melee DPS and slow.
///
/// Beastlords combine melee DPS with pet management and debuffs:
/// - Keep pet attacking current target
/// - Apply slow to targets (priority debuff)
/// - Melee DPS alongside pet
/// - Pet heals when pet HP is low
pub struct BeastlordStrategy {
    class_id: u8,
}

impl BeastlordStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    fn nearest_enemy<'a>(
        &self,
        player: &SpawnData,
        enemies: &'a [SpawnData],
    ) -> Option<&'a SpawnData> {
        let player_pos = Waypoint::new(player.x, player.y, player.z);
        enemies.iter().min_by(|a, b| {
            let dist_a = player_pos.distance_2d(&Waypoint::new(a.x, a.y, a.z));
            let dist_b = player_pos.distance_2d(&Waypoint::new(b.x, b.y, b.z));
            dist_a
                .partial_cmp(&dist_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

impl ClassStrategy for BeastlordStrategy {
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
        // Priority: slow > pet heal > DPS spells
        ctx.config.spells.iter().max_by_key(|s| s.priority).cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        if let Some(target) = ctx.target {
            tracing::info!(
                target_id = target.spawn_id,
                target_name = %target.name,
                "Beastlord engaging with pet"
            );
        }
    }

    fn aoe_threshold(&self) -> u8 {
        3
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsMelee
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beastlord_role_is_melee_dps() {
        let bl = BeastlordStrategy::new(15);
        assert!(matches!(bl.role(), CombatRole::DpsMelee));
    }
}
