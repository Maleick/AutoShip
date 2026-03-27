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
