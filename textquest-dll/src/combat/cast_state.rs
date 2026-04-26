use std::time::{Duration, SystemTime};

/// Tracks the current state of the player's casting cycle.
/// Transitions: Idle → Memorizing → Casting → Recovery → Idle
#[derive(Clone, Debug, PartialEq)]
pub enum CastState {
    /// Player is not casting and not in recovery.
    Idle,

    /// Player is memorizing a spell (MQ2 phase).
    Memorizing {
        /// Spell ID being memorized.
        spell_id: u32,
        /// When the memorization started.
        started_at: SystemTime,
    },

    /// Player is actively casting a spell.
    Casting {
        /// Spell ID being cast.
        spell_id: u32,
        /// Target ID for the spell.
        target_id: u32,
        /// When the cast started.
        started_at: SystemTime,
    },

    /// Player cast finished but is in recovery (global cooldown / spell recast delay).
    Recovery {
        /// When recovery started.
        started_at: SystemTime,
        /// Expected duration of recovery.
        duration: Duration,
    },
}

impl CastState {
    /// Create a new idle state.
    pub fn new() -> Self {
        Self::Idle
    }

    /// Transition to memorizing state.
    pub fn start_memorizing(&mut self, spell_id: u32) {
        *self = Self::Memorizing {
            spell_id,
            started_at: SystemTime::now(),
        };
    }

    /// Transition to casting state (from memorizing or other).
    pub fn start_casting(&mut self, spell_id: u32, target_id: u32) {
        *self = Self::Casting {
            spell_id,
            target_id,
            started_at: SystemTime::now(),
        };
    }

    /// Transition to recovery state.
    pub fn start_recovery(&mut self, duration: Duration) {
        *self = Self::Recovery {
            started_at: SystemTime::now(),
            duration,
        };
    }

    /// Return to idle state.
    pub fn go_idle(&mut self) {
        *self = Self::Idle;
    }

    /// Check if actively casting a spell.
    pub fn is_casting(&self) -> bool {
        matches!(self, Self::Casting { .. })
    }

    /// Check if in recovery (global cooldown).
    pub fn is_recovering(&self) -> bool {
        matches!(self, Self::Recovery { .. })
    }

    /// Check if idle and ready to cast.
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Check if memorizing a spell.
    pub fn is_memorizing(&self) -> bool {
        matches!(self, Self::Memorizing { .. })
    }

    /// Get the spell ID if currently casting or memorizing.
    pub fn current_spell_id(&self) -> Option<u32> {
        match self {
            Self::Casting { spell_id, .. } => Some(*spell_id),
            Self::Memorizing { spell_id, .. } => Some(*spell_id),
            _ => None,
        }
    }

    /// Get the target ID if currently casting.
    pub fn current_target_id(&self) -> Option<u32> {
        match self {
            Self::Casting { target_id, .. } => Some(*target_id),
            _ => None,
        }
    }

    /// Update state based on elapsed time.
    /// Call this every frame to age out casting/recovery states.
    pub fn tick(&mut self, max_cast_duration: Duration, recovery_duration: Duration) {
        match self {
            Self::Casting {
                started_at,
                spell_id,
                target_id,
            } => {
                if let Ok(elapsed) = started_at.elapsed() {
                    if elapsed > max_cast_duration {
                        // Cast timed out, go to recovery
                        tracing::warn!(
                            spell_id,
                            elapsed_ms = elapsed.as_millis(),
                            "Cast timeout exceeded"
                        );
                        self.start_recovery(recovery_duration);
                    }
                }
            }

            Self::Recovery {
                started_at,
                duration,
            } => {
                if let Ok(elapsed) = started_at.elapsed() {
                    if elapsed >= *duration {
                        // Recovery finished, go idle
                        self.go_idle();
                    }
                }
            }

            Self::Memorizing {
                started_at,
                spell_id,
            } => {
                if let Ok(elapsed) = started_at.elapsed() {
                    if elapsed > max_cast_duration {
                        // Memorization timed out, go idle
                        tracing::warn!(
                            spell_id,
                            elapsed_ms = elapsed.as_millis(),
                            "Memorization timeout exceeded"
                        );
                        self.go_idle();
                    }
                }
            }

            Self::Idle => {}
        }
    }

    /// Get time elapsed in the current state (if applicable).
    pub fn elapsed(&self) -> Option<Duration> {
        let started_at = match self {
            Self::Casting { started_at, .. }
            | Self::Memorizing { started_at, .. }
            | Self::Recovery { started_at, .. } => started_at,
            Self::Idle => return None,
        };

        started_at.elapsed().ok()
    }
}

impl Default for CastState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_idle() {
        let state = CastState::new();
        assert!(state.is_idle());
        assert!(!state.is_casting());
        assert!(!state.is_recovering());
        assert!(!state.is_memorizing());
    }

    #[test]
    fn transition_to_memorizing() {
        let mut state = CastState::new();
        state.start_memorizing(42);
        assert!(state.is_memorizing());
        assert_eq!(state.current_spell_id(), Some(42));
    }

    #[test]
    fn transition_idle_to_casting() {
        let mut state = CastState::new();
        state.start_casting(42, 100);
        assert!(state.is_casting());
        assert!(!state.is_idle());
        assert_eq!(state.current_spell_id(), Some(42));
        assert_eq!(state.current_target_id(), Some(100));
    }

    #[test]
    fn transition_memorizing_to_casting() {
        let mut state = CastState::new();
        state.start_memorizing(42);
        assert!(state.is_memorizing());

        state.start_casting(42, 100);
        assert!(state.is_casting());
        assert!(!state.is_memorizing());
    }

    #[test]
    fn transition_casting_to_recovery() {
        let mut state = CastState::new();
        state.start_casting(42, 100);
        let recovery_duration = Duration::from_millis(1500);
        state.start_recovery(recovery_duration);
        assert!(state.is_recovering());
        assert!(!state.is_casting());
    }

    #[test]
    fn transition_recovery_to_idle() {
        let mut state = CastState::new();
        let recovery_duration = Duration::from_millis(10);
        state.start_recovery(recovery_duration);
        assert!(state.is_recovering());

        // Wait for recovery to expire
        std::thread::sleep(Duration::from_millis(20));
        state.tick(Duration::from_secs(10), recovery_duration);
        assert!(state.is_idle());
    }

    #[test]
    fn casting_timeout() {
        let mut state = CastState::new();
        state.start_casting(42, 100);
        assert!(state.is_casting());

        let max_cast_duration = Duration::from_millis(10);
        let recovery_duration = Duration::from_secs(1);

        // Wait for cast to timeout
        std::thread::sleep(Duration::from_millis(20));
        state.tick(max_cast_duration, recovery_duration);

        // Should transition to recovery
        assert!(state.is_recovering());
    }

    #[test]
    fn memorizing_timeout() {
        let mut state = CastState::new();
        state.start_memorizing(42);
        assert!(state.is_memorizing());

        let max_duration = Duration::from_millis(10);
        let recovery_duration = Duration::from_secs(1);

        // Wait for memorization to timeout
        std::thread::sleep(Duration::from_millis(20));
        state.tick(max_duration, recovery_duration);

        // Should transition to idle
        assert!(state.is_idle());
    }

    #[test]
    fn elapsed_time() {
        let mut state = CastState::new();
        state.start_casting(42, 100);

        std::thread::sleep(Duration::from_millis(50));

        if let Some(elapsed) = state.elapsed() {
            assert!(elapsed >= Duration::from_millis(50));
        } else {
            panic!("Expected elapsed time");
        }
    }

    #[test]
    fn idle_no_elapsed() {
        let state = CastState::new();
        assert_eq!(state.elapsed(), None);
    }

    #[test]
    fn target_id_only_in_casting() {
        let mut state = CastState::new();
        assert_eq!(state.current_target_id(), None);

        state.start_memorizing(42);
        assert_eq!(state.current_target_id(), None);

        state.start_casting(42, 200);
        assert_eq!(state.current_target_id(), Some(200));

        state.start_recovery(Duration::from_secs(1));
        assert_eq!(state.current_target_id(), None);
    }

    #[test]
    fn go_idle_resets_state() {
        let mut state = CastState::new();
        state.start_casting(42, 100);
        assert!(state.is_casting());

        state.go_idle();
        assert!(state.is_idle());
        assert_eq!(state.current_spell_id(), None);
        assert_eq!(state.current_target_id(), None);
    }
}
