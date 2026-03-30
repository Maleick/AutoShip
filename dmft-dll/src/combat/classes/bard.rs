use dmft_common::combat::{CombatRole, SpellEntry};
use dmft_common::nav::Waypoint;
use dmft_common::types::SpawnData;

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Bard strategy: melody twist engine that cycles through songs.
///
/// Bards are unique — they don't cast spells with normal cast times.
/// Instead, they "twist" songs: start casting the next song while the
/// previous one's effect is still active. The twist cycle should be
/// ~3 seconds per song in the rotation.
///
/// Combat role varies: can pull (with resist songs), CC (with mez song),
/// provide haste/regen buffs, or add DPS with DoT songs.
pub struct BardStrategy {
    class_id: u8,
    /// Current position in the melody twist rotation (0-indexed).
    twist_index: usize,
    /// Tick when the last song was started.
    last_twist_tick: u32,
    /// Number of ticks between twists (~90 ticks = ~3 seconds at 30 tps).
    twist_interval: u32,
}

impl BardStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            twist_index: 0,
            // Start at max so the first twist fires immediately on first tick.
            last_twist_tick: u32::MAX - 200,
            twist_interval: 90, // ~3 seconds between twists
        }
    }

    /// Find nearest enemy for pulling/targeting.
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

impl ClassStrategy for BardStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // Bards assist the MA when in combat.
        if ctx.in_combat {
            ctx.target.map(|t| t.spawn_id)
        } else {
            self.nearest_enemy(ctx.player, ctx.nearby_enemies)
                .map(|s| s.spawn_id)
        }
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        // Melody twist engine: cycle through configured songs.
        // Only advance if enough ticks have passed since last twist.
        if ctx.tick.wrapping_sub(self.last_twist_tick) < self.twist_interval {
            return None; // Not time to twist yet
        }

        let spells = &ctx.config.spells;
        if spells.is_empty() {
            return None;
        }

        // Get the next song in the rotation.
        let idx = self.twist_index % spells.len();
        Some(spells[idx].clone())
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true // Bards assist the MA
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        // Reset twist rotation on new engagement.
        self.twist_index = 0;
        self.last_twist_tick = ctx.tick;

        if let Some(target) = ctx.target {
            tracing::info!(
                target_id = target.spawn_id,
                target_name = %target.name,
                songs = ctx.config.spells.len(),
                "Bard engaging — starting melody twist"
            );
        }
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        // Advance twist index after each spell cast (called per tick).
        // The actual twist advance happens here to keep select_spell pure.
        self.twist_index = (self.twist_index + 1) % ctx.config.spells.len().max(1);
        self.last_twist_tick = ctx.tick;
    }

    fn aoe_threshold(&self) -> u8 {
        3 // Bards have good AoE songs
    }

    fn role(&self) -> CombatRole {
        CombatRole::Support
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spell(id: i32, name: &str, priority: u8) -> SpellEntry {
        SpellEntry {
            slot: 1,
            spell_id: id,
            name: name.to_string(),
            min_mana_pct: 0.0,
            priority,
            is_aoe: false,
        }
    }

    #[test]
    fn bard_twist_cycles_through_songs() {
        let mut bard = BardStrategy::new(8);
        let player = SpawnData::default();
        let songs = vec![
            make_spell(100, "Selo's", 1),
            make_spell(101, "Chant", 2),
            make_spell(102, "Anthem", 3),
        ];
        let config = dmft_common::combat::CombatConfig {
            spells: songs,
            ..Default::default()
        };

        // First twist at tick 0
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
        };
        let spell = bard.select_spell(&ctx);
        assert!(spell.is_some());
        assert_eq!(spell.unwrap().spell_id, 100); // First song

        // Advance twist
        bard.on_action_complete(&ctx);

        // Second twist at tick 181 (90 ticks after on_kill set last_twist_tick to 91)
        let ctx = CombatContext { tick: 182, ..ctx };
        let spell = bard.select_spell(&ctx);
        assert!(spell.is_some());
        assert_eq!(spell.unwrap().spell_id, 101); // Second song
    }

    #[test]
    fn bard_role_is_support() {
        let bard = BardStrategy::new(8);
        assert!(matches!(bard.role(), CombatRole::Support));
    }
}
