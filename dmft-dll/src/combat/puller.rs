use std::collections::HashSet;

use dmft_common::combat::PullMethod;
use super::strategy::CombatContext;

/// States for the pulling sub-FSM.
#[derive(Debug)]
enum PullerState {
    /// Ready to pull next mob.
    Ready,
    /// Currently pulling a mob.
    Pulling { target_id: u32, started_tick: u32 },
    /// Waiting between pulls (chain pull cooldown).
    Waiting { ticks_remaining: u32 },
    /// Returning to camp after pull.
    Returning,
}

/// Manages the pulling cycle as a sub-state within the combat FSM.
pub struct Puller {
    state: PullerState,
    method: PullMethod,
    pull_range: f32,
    camp_range: f32,
    recently_pulled: HashSet<u32>,
}

impl Puller {
    pub fn new(method: PullMethod) -> Self {
        Self {
            state: PullerState::Ready,
            method,
            pull_range: 200.0,
            camp_range: 100.0,
            recently_pulled: HashSet::new(),
        }
    }

    pub fn is_pulling(&self) -> bool {
        matches!(self.state, PullerState::Pulling { .. })
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state, PullerState::Ready)
    }

    /// Select the next pull target: closest unengaged NPC within pull range
    /// that hasn't been pulled recently.
    pub fn next_target(&self, ctx: &CombatContext) -> Option<u32> {
        let player_pos = dmft_common::nav::Waypoint::new(ctx.player.x, ctx.player.y, ctx.player.z);
        ctx.nearby_enemies.iter()
            .filter(|npc| !self.recently_pulled.contains(&npc.spawn_id))
            .map(|npc| {
                let dist = player_pos.distance_2d(&dmft_common::nav::Waypoint::new(npc.x, npc.y, npc.z));
                (npc.spawn_id, dist)
            })
            .filter(|(_, dist)| *dist < self.pull_range)
            .min_by(|(_, da), (_, db)| da.partial_cmp(db).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| id)
    }

    /// Advance the puller state machine one tick.
    pub fn tick(&mut self, ctx: &CombatContext) {
        match &mut self.state {
            PullerState::Ready => {
                // Caller should check next_target() and initiate pull
            }
            PullerState::Pulling { target_id, started_tick } => {
                let elapsed = ctx.tick - *started_tick;
                let tid = *target_id;
                // After 60 ticks (~3 sec), assume pull landed or failed
                if elapsed > 60 {
                    self.recently_pulled.insert(tid);
                    self.state = PullerState::Returning;
                    tracing::debug!(target_id = tid, "Pull complete, returning");
                }
            }
            PullerState::Waiting { ticks_remaining } => {
                if *ticks_remaining == 0 {
                    self.state = PullerState::Ready;
                } else {
                    *ticks_remaining -= 1;
                }
            }
            PullerState::Returning => {
                // Once at camp (caller checks distance), transition to waiting
                self.state = PullerState::Waiting { ticks_remaining: 20 };
            }
        }
    }

    /// Start a pull on a specific target.
    pub fn start_pull(&mut self, target_id: u32, current_tick: u32) {
        tracing::info!(target_id, "Starting pull");
        self.state = PullerState::Pulling {
            target_id,
            started_tick: current_tick,
        };
    }

    pub fn method(&self) -> &PullMethod { &self.method }

    /// Clear recently pulled list (e.g., when camp changes).
    pub fn reset(&mut self) {
        self.recently_pulled.clear();
        self.state = PullerState::Ready;
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use dmft_common::combat::CombatConfig;
    use dmft_common::types::SpawnData;

    static DEFAULT_CONFIG: std::sync::LazyLock<CombatConfig> =
        std::sync::LazyLock::new(CombatConfig::default);

    fn make_ctx_with_enemies(
        player: &SpawnData,
        enemies: &[SpawnData],
        tick: u32,
    ) -> CombatContext<'_> {
        CombatContext {
            player,
            target: None,
            nearby_enemies: enemies,
            group_members: &[],
            config: &DEFAULT_CONFIG,
            tick,
            in_combat: false,
            ch_chain_slot: None,
        }
    }

    #[test]
    fn new_puller_is_ready() {
        let puller = Puller::new(PullMethod::BowPull);
        assert!(puller.is_ready());
        assert!(!puller.is_pulling());
    }

    #[test]
    fn start_pull_transitions_to_pulling() {
        let mut puller = Puller::new(PullMethod::BowPull);
        puller.start_pull(42, 100);
        assert!(puller.is_pulling());
        assert!(!puller.is_ready());
    }

    #[test]
    fn pull_completes_after_60_ticks() {
        let mut puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        puller.start_pull(42, 100);

        // At tick 160 (60 ticks elapsed), should complete
        let ctx = make_ctx_with_enemies(&player, &[], 161);
        puller.tick(&ctx);
        assert!(!puller.is_pulling());
        // Should be in Returning state now
    }

    #[test]
    fn returning_transitions_to_waiting() {
        let mut puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        puller.start_pull(42, 100);

        // Complete the pull
        let ctx = make_ctx_with_enemies(&player, &[], 161);
        puller.tick(&ctx);

        // Returning → Waiting
        let ctx2 = make_ctx_with_enemies(&player, &[], 162);
        puller.tick(&ctx2);
        assert!(!puller.is_ready());
        assert!(!puller.is_pulling());
    }

    #[test]
    fn waiting_transitions_to_ready() {
        let mut puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        puller.start_pull(42, 100);

        // Complete pull → Returning
        let ctx = make_ctx_with_enemies(&player, &[], 161);
        puller.tick(&ctx);

        // Returning → Waiting(20)
        let ctx2 = make_ctx_with_enemies(&player, &[], 162);
        puller.tick(&ctx2);

        // Tick through waiting period (20 ticks)
        for i in 0..21 {
            let ctx3 = make_ctx_with_enemies(&player, &[], 163 + i);
            puller.tick(&ctx3);
        }
        assert!(puller.is_ready());
    }

    #[test]
    fn next_target_closest_unengaged() {
        let puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 100.0,
                y: 0.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 50.0,
                y: 0.0,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx_with_enemies(&player, &enemies, 0);
        assert_eq!(puller.next_target(&ctx), Some(2)); // closest
    }

    #[test]
    fn next_target_filters_recently_pulled() {
        let mut puller = Puller::new(PullMethod::BowPull);
        puller.recently_pulled.insert(2);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 100.0,
                y: 0.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 50.0,
                y: 0.0,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx_with_enemies(&player, &enemies, 0);
        assert_eq!(puller.next_target(&ctx), Some(1)); // 2 is recently pulled
    }

    #[test]
    fn next_target_filters_out_of_range() {
        let puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        let enemies = vec![SpawnData {
            spawn_id: 1,
            x: 500.0, // > pull_range (200)
            y: 0.0,
            ..SpawnData::default()
        }];
        let ctx = make_ctx_with_enemies(&player, &enemies, 0);
        assert!(puller.next_target(&ctx).is_none());
    }

    #[test]
    fn next_target_empty_enemies() {
        let puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        let ctx = make_ctx_with_enemies(&player, &[], 0);
        assert!(puller.next_target(&ctx).is_none());
    }

    #[test]
    fn next_target_all_recently_pulled() {
        let mut puller = Puller::new(PullMethod::BowPull);
        puller.recently_pulled.insert(1);
        puller.recently_pulled.insert(2);
        let player = SpawnData::default();
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 50.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 60.0,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx_with_enemies(&player, &enemies, 0);
        assert!(puller.next_target(&ctx).is_none());
    }

    #[test]
    fn reset_clears_state() {
        let mut puller = Puller::new(PullMethod::BowPull);
        puller.recently_pulled.insert(1);
        puller.start_pull(42, 100);
        assert!(puller.is_pulling());

        puller.reset();
        assert!(puller.is_ready());
        assert!(!puller.is_pulling());
    }

    #[test]
    fn method_returns_configured_method() {
        let puller = Puller::new(PullMethod::BowPull);
        assert!(matches!(puller.method(), PullMethod::BowPull));

        let puller2 = Puller::new(PullMethod::SpellPull { spell_slot: 3 });
        if let PullMethod::SpellPull { spell_slot } = puller2.method() {
            assert_eq!(*spell_slot, 3);
        } else {
            panic!("expected SpellPull");
        }
    }

    #[test]
    fn pulled_target_added_to_recently_pulled() {
        let mut puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        puller.start_pull(42, 100);

        // Complete pull
        let ctx = make_ctx_with_enemies(&player, &[], 161);
        puller.tick(&ctx);

        // Now 42 should be in recently_pulled
        let enemies = vec![SpawnData {
            spawn_id: 42,
            x: 50.0,
            ..SpawnData::default()
        }];
        let ctx2 = make_ctx_with_enemies(&player, &enemies, 162);
        assert!(puller.next_target(&ctx2).is_none());
    }
}
