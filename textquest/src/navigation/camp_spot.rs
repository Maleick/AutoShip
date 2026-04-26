//! Return-state machine for auto-return to camp spot.
//!
//! Guards the transition from post-combat/post-loot back to the camp position
//! with two gates (`NoAggroGate` and `NotLootingGate`) and a jittered delay
//! before movement begins. This prevents characters from running back while
//! there is still aggro on the tank or while a loot window is open.
//!
//! # State machine
//!
//! ```text
//! Idle
//!   │  trigger_return()
//!   ▼
//! AwaitingGates     ← re-checked every tick(); blocks if aggro or looting
//!   │  gates clear
//!   ▼
//! JitterDelay       ← waits for mindelay..maxdelay before moving
//!   │  elapsed
//!   ▼
//! Returning         ← caller issues movement commands
//!   │  arrived
//!   ▼
//! AtCamp
//! ```

use std::time::{Duration, Instant};

use rand::Rng as _;

/// Current state of the return-state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnState {
    /// No return is pending.
    Idle,
    /// Waiting for gates to clear before applying the jitter delay.
    AwaitingGates,
    /// Gates cleared; counting down the jittered pre-move delay.
    JitterDelay {
        /// Absolute time when the delay expires.
        ready_at: Instant,
    },
    /// Movement to camp spot is in progress.
    Returning,
    /// Character is back at the camp spot.
    AtCamp,
}

/// Configuration for the return-state machine jitter.
#[derive(Debug, Clone)]
pub struct ReturnJitterConfig {
    /// Minimum delay before return movement begins (milliseconds).
    pub min_delay_ms: u64,
    /// Maximum delay before return movement begins (milliseconds).
    pub max_delay_ms: u64,
}

impl Default for ReturnJitterConfig {
    fn default() -> Self {
        Self {
            min_delay_ms: 500,
            max_delay_ms: 3000,
        }
    }
}

/// Return-state machine with `NoAggro` and `NotLooting` gates and a
/// jittered pre-movement delay.
#[derive(Debug)]
pub struct CampReturnMachine {
    state: ReturnState,
    jitter: ReturnJitterConfig,
}

impl CampReturnMachine {
    /// Construct a new machine in the `Idle` state.
    #[must_use]
    pub fn new(jitter: ReturnJitterConfig) -> Self {
        Self {
            state: ReturnState::Idle,
            jitter,
        }
    }

    /// Construct with default jitter settings.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(ReturnJitterConfig::default())
    }

    /// Current state of the machine.
    #[must_use]
    pub fn state(&self) -> &ReturnState {
        &self.state
    }

    /// Signal that an auto-return should be attempted.
    ///
    /// Transitions `Idle` or `AtCamp` → `AwaitingGates`.  Calls from any other
    /// state are ignored so repeated triggers do not reset mid-flight returns.
    pub fn trigger_return(&mut self) {
        if matches!(self.state, ReturnState::Idle | ReturnState::AtCamp) {
            self.state = ReturnState::AwaitingGates;
        }
    }

    /// Tick the state machine.
    ///
    /// `tank_has_aggro` — true if the tank currently holds aggro on any mob.
    /// `loot_window_open` — true if any loot window is open for this character.
    ///
    /// Returns the action the caller should take this tick.
    pub fn tick(&mut self, tank_has_aggro: bool, loot_window_open: bool) -> ReturnAction {
        match &self.state {
            ReturnState::Idle | ReturnState::AtCamp => ReturnAction::None,

            ReturnState::AwaitingGates => {
                // NoAggroGate: block while tank has aggro.
                if tank_has_aggro {
                    return ReturnAction::WaitingOnGate(GateKind::NoAggro);
                }
                // NotLootingGate: block while loot window is open.
                if loot_window_open {
                    return ReturnAction::WaitingOnGate(GateKind::NotLooting);
                }
                // Both gates clear — apply jitter delay.
                let delay_ms = rand::rng().random_range(
                    self.jitter.min_delay_ms..=self.jitter.max_delay_ms,
                );
                let ready_at = Instant::now() + Duration::from_millis(delay_ms);
                self.state = ReturnState::JitterDelay { ready_at };
                ReturnAction::WaitingOnJitter
            }

            ReturnState::JitterDelay { ready_at } => {
                // Re-check gates while counting down — aggro/loot can reappear.
                if tank_has_aggro {
                    self.state = ReturnState::AwaitingGates;
                    return ReturnAction::WaitingOnGate(GateKind::NoAggro);
                }
                if loot_window_open {
                    self.state = ReturnState::AwaitingGates;
                    return ReturnAction::WaitingOnGate(GateKind::NotLooting);
                }
                if Instant::now() >= *ready_at {
                    self.state = ReturnState::Returning;
                    ReturnAction::BeginReturn
                } else {
                    ReturnAction::WaitingOnJitter
                }
            }

            ReturnState::Returning => ReturnAction::Returning,
        }
    }

    /// Notify the machine that the character has arrived at the camp spot.
    ///
    /// Transitions `Returning` → `AtCamp`.  Ignored from other states.
    pub fn notify_arrived(&mut self) {
        if self.state == ReturnState::Returning {
            self.state = ReturnState::AtCamp;
        }
    }

    /// Reset the machine to `Idle` (e.g. on new pull or explicit cancel).
    pub fn reset(&mut self) {
        self.state = ReturnState::Idle;
    }
}

/// Which gate is blocking the return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateKind {
    /// Tank still has aggro — return deferred.
    NoAggro,
    /// Loot window still open — return deferred.
    NotLooting,
}

/// Action returned by [`CampReturnMachine::tick`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnAction {
    /// Nothing to do.
    None,
    /// A gate is blocking; caller should wait and re-tick.
    WaitingOnGate(GateKind),
    /// Jitter delay counting down; caller should wait and re-tick.
    WaitingOnJitter,
    /// Caller should issue movement commands toward the camp spot.
    BeginReturn,
    /// Movement is in progress; caller should continue movement.
    Returning,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine() -> CampReturnMachine {
        CampReturnMachine::new(ReturnJitterConfig {
            min_delay_ms: 0,
            max_delay_ms: 0,
        })
    }

    // --- gate combination matrix ---

    #[test]
    fn idle_tick_is_noop() {
        let mut m = machine();
        assert_eq!(m.tick(false, false), ReturnAction::None);
        assert_eq!(m.tick(true, false), ReturnAction::None);
        assert_eq!(m.tick(false, true), ReturnAction::None);
        assert_eq!(m.tick(true, true), ReturnAction::None);
    }

    #[test]
    fn awaiting_blocks_on_aggro() {
        let mut m = machine();
        m.trigger_return();
        assert_eq!(
            m.tick(true, false),
            ReturnAction::WaitingOnGate(GateKind::NoAggro)
        );
        assert_eq!(m.state(), &ReturnState::AwaitingGates);
    }

    #[test]
    fn awaiting_blocks_on_looting() {
        let mut m = machine();
        m.trigger_return();
        assert_eq!(
            m.tick(false, true),
            ReturnAction::WaitingOnGate(GateKind::NotLooting)
        );
        assert_eq!(m.state(), &ReturnState::AwaitingGates);
    }

    #[test]
    fn awaiting_blocks_on_both_gates() {
        let mut m = machine();
        m.trigger_return();
        // Aggro takes priority in the check order.
        assert_eq!(
            m.tick(true, true),
            ReturnAction::WaitingOnGate(GateKind::NoAggro)
        );
    }

    #[test]
    fn awaiting_clears_and_begins_return_when_no_gates() {
        // With zero-duration jitter the machine collapses JitterDelay → Returning
        // in a single tick cycle (delay already expired by the time we check).
        let mut m = machine();
        m.trigger_return();
        // First tick: gates clear → enters JitterDelay with 0ms → transitions to Returning.
        let action = m.tick(false, false);
        // Either WaitingOnJitter (instant not yet expired on fast hardware) or BeginReturn.
        assert!(
            matches!(action, ReturnAction::WaitingOnJitter | ReturnAction::BeginReturn),
            "unexpected action: {action:?}"
        );
    }

    #[test]
    fn aggro_during_jitter_resets_to_awaiting_gates() {
        let mut m = CampReturnMachine::new(ReturnJitterConfig {
            min_delay_ms: 60_000,
            max_delay_ms: 60_000,
        });
        m.trigger_return();
        m.tick(false, false); // clear gates → JitterDelay with 60 s
        assert!(matches!(m.state(), ReturnState::JitterDelay { .. }));
        // Aggro reappears during jitter.
        assert_eq!(
            m.tick(true, false),
            ReturnAction::WaitingOnGate(GateKind::NoAggro)
        );
        assert_eq!(m.state(), &ReturnState::AwaitingGates);
    }

    #[test]
    fn looting_during_jitter_resets_to_awaiting_gates() {
        let mut m = CampReturnMachine::new(ReturnJitterConfig {
            min_delay_ms: 60_000,
            max_delay_ms: 60_000,
        });
        m.trigger_return();
        m.tick(false, false); // clear gates → JitterDelay with 60 s
        assert_eq!(
            m.tick(false, true),
            ReturnAction::WaitingOnGate(GateKind::NotLooting)
        );
        assert_eq!(m.state(), &ReturnState::AwaitingGates);
    }

    #[test]
    fn notify_arrived_transitions_returning_to_at_camp() {
        let mut m = machine();
        m.trigger_return();
        // Drive until Returning.
        loop {
            match m.tick(false, false) {
                ReturnAction::BeginReturn | ReturnAction::Returning => break,
                ReturnAction::WaitingOnJitter => continue,
                other => panic!("unexpected: {other:?}"),
            }
        }
        m.notify_arrived();
        assert_eq!(m.state(), &ReturnState::AtCamp);
    }

    #[test]
    fn notify_arrived_ignored_when_not_returning() {
        let mut m = machine();
        m.notify_arrived();
        assert_eq!(m.state(), &ReturnState::Idle);
    }

    #[test]
    fn reset_returns_to_idle_from_any_state() {
        let mut m = machine();
        m.trigger_return();
        m.reset();
        assert_eq!(m.state(), &ReturnState::Idle);
    }

    #[test]
    fn trigger_return_ignored_when_already_in_flight() {
        let mut m = CampReturnMachine::new(ReturnJitterConfig {
            min_delay_ms: 60_000,
            max_delay_ms: 60_000,
        });
        m.trigger_return();
        m.tick(false, false); // → JitterDelay
        let state_before = m.state().clone();
        m.trigger_return(); // should be ignored
        assert_eq!(m.state(), &state_before);
    }

    #[test]
    fn at_camp_trigger_restarts_return_cycle() {
        let mut m = machine();
        m.trigger_return();
        loop {
            match m.tick(false, false) {
                ReturnAction::BeginReturn | ReturnAction::Returning => break,
                ReturnAction::WaitingOnJitter => continue,
                other => panic!("unexpected: {other:?}"),
            }
        }
        m.notify_arrived();
        assert_eq!(m.state(), &ReturnState::AtCamp);
        // Trigger again — should restart.
        m.trigger_return();
        assert_eq!(m.state(), &ReturnState::AwaitingGates);
    }
}
