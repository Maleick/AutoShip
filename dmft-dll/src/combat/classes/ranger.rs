use dmft_common::combat::{CombatRole, SpellEntry};
use dmft_common::types::SpawnData;
use dmft_common::nav::Waypoint;

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Ranger strategy: ranged/melee hybrid DPS with tracking and bow pulling.
///
/// Rangers operate in two stances:
/// - **Ranged**: Use bow attacks and DoT spells from distance (default when pulling)
/// - **Melee**: Switch to melee when target is close, use kicks and backstab-style abilities
///
/// Rangers also provide: tracking (find mobs), snare (Snare/Ensnare),
/// and at higher levels, Headshot AA for trivial kills.
pub struct RangerStrategy {
    class_id: u8,
    /// Distance threshold to switch from ranged to melee (in EQ units).
    melee_range: f32,
}

impl RangerStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            melee_range: 30.0, // Switch to melee within 30 units
        }
    }

    /// Find nearest enemy for targeting.
    fn nearest_enemy<'a>(&self, player: &SpawnData, enemies: &'a [SpawnData]) -> Option<&'a SpawnData> {
        let player_pos = Waypoint::new(player.x, player.y, player.z);
        enemies.iter().min_by(|a, b| {
            let dist_a = player_pos.distance_2d(&Waypoint::new(a.x, a.y, a.z));
            let dist_b = player_pos.distance_2d(&Waypoint::new(b.x, b.y, b.z));
            dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Calculate distance to target.
    fn distance_to(&self, player: &SpawnData, target: &SpawnData) -> f32 {
        let p = Waypoint::new(player.x, player.y, player.z);
        let t = Waypoint::new(target.x, target.y, target.z);
        p.distance_2d(&t)
    }

    /// Whether we're in melee range of the target.
    fn in_melee_range(&self, ctx: &CombatContext) -> bool {
        ctx.target
            .map(|t| self.distance_to(ctx.player, t) <= self.melee_range)
            .unwrap_or(false)
    }
}

impl ClassStrategy for RangerStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // Assist MA when in combat, otherwise target nearest.
        if ctx.in_combat {
            ctx.target.map(|t| t.spawn_id)
        } else {
            self.nearest_enemy(ctx.player, ctx.nearby_enemies)
                .map(|s| s.spawn_id)
        }
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let spells = &ctx.config.spells;
        if spells.is_empty() {
            return None;
        }

        if self.in_melee_range(ctx) {
            // In melee range: prefer melee abilities (lower spell IDs or higher priority)
            spells
                .iter()
                .filter(|s| s.priority >= 5) // High priority = melee abilities
                .max_by_key(|s| s.priority)
                .or_else(|| spells.iter().max_by_key(|s| s.priority))
                .cloned()
        } else {
            // At range: prefer ranged spells (DoTs, snare, bow)
            spells
                .iter()
                .filter(|s| s.priority < 5) // Lower priority = ranged
                .max_by_key(|s| s.priority)
                .or_else(|| spells.iter().max_by_key(|s| s.priority))
                .cloned()
        }
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true // Rangers assist the MA
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        if let Some(target) = ctx.target {
            let dist = self.distance_to(ctx.player, target);
            let stance = if dist <= self.melee_range { "melee" } else { "ranged" };
            tracing::info!(
                target_id = target.spawn_id,
                target_name = %target.name,
                distance = format!("{:.0}", dist),
                stance,
                "Ranger engaging target"
            );
        }
    }

    fn on_action_complete(&mut self, _ctx: &CombatContext) {}

    fn aoe_threshold(&self) -> u8 {
        3
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsRanged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranger_role_is_dps() {
        let ranger = RangerStrategy::new(4);
        assert!(matches!(ranger.role(), CombatRole::DpsRanged));
    }

    #[test]
    fn ranger_assists_ma() {
        let ranger = RangerStrategy::new(4);
        let player = SpawnData::default();
        let config = dmft_common::combat::CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
        };
        assert!(ranger.should_assist(&ctx));
    }
}
