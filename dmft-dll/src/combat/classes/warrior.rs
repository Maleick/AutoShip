use dmft_common::combat::{CombatRole, SpellEntry};
use dmft_common::nav::Waypoint;
use dmft_common::types::SpawnData;

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Warrior strategy: main tank, selects nearest enemy, uses taunt/aggro abilities.
pub struct WarriorStrategy {
    class_id: u8,
}

impl WarriorStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    /// Find the nearest NPC from the nearby enemies list based on distance to player.
    fn nearest_enemy<'a>(&self, player: &SpawnData, enemies: &'a [SpawnData]) -> Option<&'a SpawnData> {
        let player_pos = Waypoint::new(player.x, player.y, player.z);
        enemies.iter().min_by(|a, b| {
            let dist_a = player_pos.distance_2d(&Waypoint::new(a.x, a.y, a.z));
            let dist_b = player_pos.distance_2d(&Waypoint::new(b.x, b.y, b.z));
            dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

impl ClassStrategy for WarriorStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        self.nearest_enemy(ctx.player, ctx.nearby_enemies)
            .map(|s| s.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        // Use highest-priority taunt/aggro ability from config spells list.
        // Spells are assumed sorted or we pick the highest priority.
        ctx.config
            .spells
            .iter()
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        // Tank leads, doesn't assist.
        false
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        if let Some(target) = ctx.target {
            tracing::info!(
                target_id = target.spawn_id,
                target_name = %target.name,
                "Warrior engaging target"
            );
        }
    }

    fn on_kill(&mut self, _ctx: &CombatContext) {}

    fn aoe_threshold(&self) -> u8 {
        2
    }

    fn role(&self) -> CombatRole {
        CombatRole::MainTank
    }
}
