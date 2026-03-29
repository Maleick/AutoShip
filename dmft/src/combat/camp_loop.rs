//! Camp loop FSM — orchestrates the full camp cycle:
//! camp → pull → combat → loot → return to camp → repeat.
//!
//! This ties together camp positioning, pulling, combat coordination,
//! and recovery into a single state machine.

use dmft_common::ipc::Command;
use dmft_common::types::ClientId;
use std::time::{Duration, Instant};

/// The camp loop state machine.
#[derive(Debug)]
pub struct CampLoop {
    state: CampState,
    /// When the current state was entered.
    state_entered: Instant,
    /// Puller client ID.
    puller_id: Option<ClientId>,
    /// Whether the loop is active.
    active: bool,
    /// How long to wait at camp before pulling (seconds).
    camp_delay: Duration,
    /// Max time in any single state before timeout recovery.
    state_timeout: Duration,
    /// Number of consecutive wipes.
    wipe_count: u32,
}

/// Camp loop states.
#[derive(Debug, Clone, PartialEq)]
pub enum CampState {
    /// Idle — not looping.
    Idle,
    /// At camp, buffing/medding, waiting before next pull.
    AtCamp,
    /// Puller is out pulling a mob.
    Pulling,
    /// Group is in combat.
    Fighting,
    /// Combat over, looting corpses.
    Looting,
    /// Returning to camp positions after combat/loot.
    Returning,
    /// Recovery after wipe — waiting for respawn, rezzing, regrouping.
    Recovery,
}

/// Events the camp loop can process.
#[derive(Debug)]
pub enum CampEvent {
    /// All group members are at their camp spots and ready.
    GroupReady,
    /// Puller has engaged a mob and is bringing it back.
    PullIncoming,
    /// Combat has started (mob is in camp).
    CombatStarted,
    /// All mobs are dead.
    CombatEnded,
    /// Looting is complete.
    LootingDone,
    /// All members returned to camp spots.
    ReturnedToCamp,
    /// A group member died.
    MemberDied { client_id: ClientId },
    /// All members are dead (wipe).
    GroupWiped,
    /// Recovery is complete (rezzed, regrouped).
    RecoveryComplete,
    /// External command to pause the loop.
    Pause,
    /// External command to resume the loop.
    Resume,
}

impl CampLoop {
    pub fn new() -> Self {
        Self {
            state: CampState::Idle,
            state_entered: Instant::now(),
            puller_id: None,
            active: false,
            camp_delay: Duration::from_secs(5),
            state_timeout: Duration::from_secs(120),
            wipe_count: 0,
        }
    }

    pub fn set_puller(&mut self, client_id: ClientId) {
        self.puller_id = Some(client_id);
    }

    pub fn state(&self) -> &CampState {
        &self.state
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Start the camp loop.
    pub fn start(&mut self) {
        self.active = true;
        self.transition(CampState::AtCamp);
        tracing::info!("Camp loop started");
    }

    /// Stop the camp loop.
    pub fn stop(&mut self) {
        self.active = false;
        self.transition(CampState::Idle);
        tracing::info!("Camp loop stopped");
    }

    /// Process an event and return any commands to send.
    pub fn process_event(&mut self, event: CampEvent) -> Vec<(ClientId, Command)> {
        if !self.active && !matches!(event, CampEvent::Resume) {
            return Vec::new();
        }

        let mut commands = Vec::new();

        match (&self.state, event) {
            // AtCamp → Pulling (when group is ready)
            (CampState::AtCamp, CampEvent::GroupReady) => {
                if self.state_entered.elapsed() >= self.camp_delay {
                    if let Some(puller_id) = self.puller_id {
                        tracing::info!("Camp loop: sending puller");
                        commands.push((puller_id, Command::CombatEngage { target_id: 0 }));
                        self.transition(CampState::Pulling);
                    }
                }
            }

            // Pulling → Fighting (mob is incoming)
            (CampState::Pulling, CampEvent::PullIncoming | CampEvent::CombatStarted) => {
                tracing::info!("Camp loop: pull incoming, engaging");
                self.transition(CampState::Fighting);
            }

            // Fighting → Looting (combat over)
            (CampState::Fighting, CampEvent::CombatEnded) => {
                tracing::info!("Camp loop: combat ended, looting");
                self.wipe_count = 0; // Reset wipe counter on successful kill
                self.transition(CampState::Looting);
            }

            // Looting → Returning (done looting)
            (CampState::Looting, CampEvent::LootingDone) => {
                tracing::info!("Camp loop: looting done, returning to camp");
                self.transition(CampState::Returning);
            }

            // Returning → AtCamp (everyone back)
            (CampState::Returning, CampEvent::ReturnedToCamp) => {
                tracing::info!("Camp loop: returned to camp, medding");
                self.transition(CampState::AtCamp);
            }

            // Any combat state → Recovery (wipe)
            (CampState::Fighting | CampState::Pulling, CampEvent::GroupWiped) => {
                self.wipe_count += 1;
                tracing::warn!(
                    wipe_count = self.wipe_count,
                    "Camp loop: GROUP WIPE — entering recovery"
                );
                self.transition(CampState::Recovery);

                if self.wipe_count >= 3 {
                    tracing::error!("Camp loop: 3 consecutive wipes — stopping");
                    self.stop();
                }
            }

            // Recovery → AtCamp (recovered)
            (CampState::Recovery, CampEvent::RecoveryComplete) => {
                tracing::info!("Camp loop: recovery complete, returning to camp");
                self.transition(CampState::AtCamp);
            }

            // Pause/Resume from any state
            (_, CampEvent::Pause) => {
                tracing::info!("Camp loop: paused");
                self.active = false;
            }
            (_, CampEvent::Resume) => {
                tracing::info!("Camp loop: resumed");
                self.active = true;
            }

            // Ignore unexpected transitions
            (state, event) => {
                tracing::debug!(
                    state = ?state,
                    event = ?event,
                    "Camp loop: ignoring event in current state"
                );
            }
        }

        commands
    }

    /// Check for state timeouts and trigger recovery if stuck.
    pub fn check_timeout(&mut self) -> Option<CampEvent> {
        if !self.active {
            return None;
        }

        if self.state_entered.elapsed() > self.state_timeout {
            match &self.state {
                CampState::Pulling => {
                    tracing::warn!("Camp loop: pull timed out — returning to camp");
                    self.transition(CampState::Returning);
                    Some(CampEvent::ReturnedToCamp)
                }
                CampState::Fighting => {
                    tracing::warn!("Camp loop: combat timed out — possible stuck fight");
                    None // Don't auto-recover from combat — might be a hard mob
                }
                CampState::Looting => {
                    tracing::warn!("Camp loop: loot timed out — returning to camp");
                    self.transition(CampState::Returning);
                    Some(CampEvent::ReturnedToCamp)
                }
                CampState::Recovery => {
                    tracing::warn!("Camp loop: recovery timed out — stopping");
                    self.stop();
                    None
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn transition(&mut self, new_state: CampState) {
        tracing::debug!(
            from = ?self.state,
            to = ?new_state,
            "Camp loop state transition"
        );
        self.state = new_state;
        self.state_entered = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camp_loop_starts_idle() {
        let cl = CampLoop::new();
        assert!(!cl.is_active());
        assert_eq!(cl.state(), &CampState::Idle);
    }

    #[test]
    fn camp_loop_start_transitions_to_at_camp() {
        let mut cl = CampLoop::new();
        cl.start();
        assert!(cl.is_active());
        assert_eq!(cl.state(), &CampState::AtCamp);
    }

    #[test]
    fn camp_loop_wipe_recovery() {
        let mut cl = CampLoop::new();
        cl.set_puller(1);
        cl.start();

        // Simulate: AtCamp → Pulling → Fighting → Wipe → Recovery
        cl.transition(CampState::Fighting);
        cl.process_event(CampEvent::GroupWiped);
        assert_eq!(cl.state(), &CampState::Recovery);
        assert_eq!(cl.wipe_count, 1);

        // Recovery complete → back to AtCamp
        cl.process_event(CampEvent::RecoveryComplete);
        assert_eq!(cl.state(), &CampState::AtCamp);
    }

    #[test]
    fn camp_loop_three_wipes_stops() {
        let mut cl = CampLoop::new();
        cl.set_puller(1);
        cl.start();

        for _ in 0..3 {
            cl.transition(CampState::Fighting);
            cl.process_event(CampEvent::GroupWiped);
            if cl.is_active() {
                cl.process_event(CampEvent::RecoveryComplete);
            }
        }

        assert!(!cl.is_active(), "Should stop after 3 consecutive wipes");
    }

    #[test]
    fn camp_loop_successful_kill_resets_wipe_count() {
        let mut cl = CampLoop::new();
        cl.set_puller(1);
        cl.start();

        // Wipe once
        cl.transition(CampState::Fighting);
        cl.process_event(CampEvent::GroupWiped);
        assert_eq!(cl.wipe_count, 1);
        cl.process_event(CampEvent::RecoveryComplete);

        // Successful kill
        cl.transition(CampState::Fighting);
        cl.process_event(CampEvent::CombatEnded);
        assert_eq!(cl.wipe_count, 0);
    }

    #[test]
    fn camp_loop_pause_resume() {
        let mut cl = CampLoop::new();
        cl.start();
        assert!(cl.is_active());

        cl.process_event(CampEvent::Pause);
        assert!(!cl.is_active());

        cl.process_event(CampEvent::Resume);
        assert!(cl.is_active());
    }
}
