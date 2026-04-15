//! Session and group control model for the orchestrator.
//!
//! `SessionControl` tracks per-session routing scope, group membership, and
//! active/paused state. Commands arrive via the IPC layer as
//! `SessionControlCommand` variants and are processed by `apply_command`.

use textquest_common::{ipc::SessionControlCommand, routing::RoutingScope};

/// Lifecycle state of a managed EQ session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session is active and accepting commands.
    Active,
    /// Session has been paused by the operator — commands are queued or
    /// dropped.
    Paused,
    /// Session has entered an error state and requires intervention.
    Error,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Paused => write!(f, "Paused"),
            Self::Error => write!(f, "Error"),
        }
    }
}

/// Per-session control record maintained by the orchestrator.
///
/// Each registered EQ client has a `SessionControl` that tracks its group
/// assignment, routing scope, and operational state.  The orchestrator
/// consults these records when dispatching commands.
#[derive(Debug, Clone)]
pub struct SessionControl {
    /// Unique session identifier (typically the OS process ID).
    pub session_id: u32,
    /// Group this session belongs to (1-based, 0 = ungrouped).
    pub group_id: u8,
    /// Current routing scope for outbound commands to this session.
    pub routing_scope: RoutingScope,
    /// Operational state of this session.
    pub state: SessionState,
}

impl SessionControl {
    /// Create a new `SessionControl` in the `Active` state with `AllSession`
    /// routing scope.
    #[must_use]
    pub fn new(session_id: u32) -> Self {
        Self {
            session_id,
            group_id: 0,
            routing_scope: RoutingScope::AllSession,
            state: SessionState::Active,
        }
    }

    /// Create a new `SessionControl` with an explicit group assignment.
    #[must_use]
    pub fn with_group(session_id: u32, group_id: u8) -> Self {
        Self {
            session_id,
            group_id,
            routing_scope: RoutingScope::Group {
                group_id,
                label: format!("G{group_id}"),
            },
            state: SessionState::Active,
        }
    }

    /// Apply a `SessionControlCommand` to this session record.
    ///
    /// Returns `true` when the command changed the session state or scope,
    /// `false` when it was a no-op (e.g. pausing an already-paused session).
    pub fn apply_command(&mut self, cmd: &SessionControlCommand) -> bool {
        match cmd {
            SessionControlCommand::Pause => {
                if self.state == SessionState::Active {
                    self.state = SessionState::Paused;
                    true
                } else {
                    false
                }
            }
            SessionControlCommand::Resume => {
                if self.state == SessionState::Paused {
                    self.state = SessionState::Active;
                    true
                } else {
                    false
                }
            }
            SessionControlCommand::SetGroup { group_id } => {
                let new_scope = if *group_id == 0 {
                    RoutingScope::AllSession
                } else {
                    RoutingScope::Group {
                        group_id: *group_id,
                        label: format!("G{group_id}"),
                    }
                };
                let changed = self.group_id != *group_id || self.routing_scope != new_scope;
                self.group_id = *group_id;
                self.routing_scope = new_scope;
                changed
            }
            SessionControlCommand::BroadcastAll => {
                let changed = self.routing_scope != RoutingScope::AllSession;
                self.routing_scope = RoutingScope::AllSession;
                changed
            }
        }
    }

    /// Returns `true` when this session is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state == SessionState::Active
    }

    /// Returns `true` when this session is paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.state == SessionState::Paused
    }

    /// Mark this session as errored.
    pub fn set_error(&mut self) {
        self.state = SessionState::Error;
    }

    /// Clear an error state back to `Active`.
    ///
    /// Only transitions from `Error`; has no effect on `Active` or `Paused`.
    pub fn clear_error(&mut self) -> bool {
        if self.state == SessionState::Error {
            self.state = SessionState::Active;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::ipc::SessionControlCommand;

    #[test]
    fn new_session_is_active() {
        let sc = SessionControl::new(1234);
        assert_eq!(sc.session_id, 1234);
        assert_eq!(sc.state, SessionState::Active);
        assert_eq!(sc.group_id, 0);
        assert!(sc.is_active());
        assert!(!sc.is_paused());
    }

    #[test]
    fn with_group_sets_group_and_scope() {
        let sc = SessionControl::with_group(42, 3);
        assert_eq!(sc.group_id, 3);
        assert!(matches!(
            sc.routing_scope,
            RoutingScope::Group { group_id: 3, .. }
        ));
    }

    #[test]
    fn pause_transitions_active_to_paused() {
        let mut sc = SessionControl::new(1);
        let changed = sc.apply_command(&SessionControlCommand::Pause);
        assert!(changed);
        assert_eq!(sc.state, SessionState::Paused);
    }

    #[test]
    fn pause_noop_when_already_paused() {
        let mut sc = SessionControl::new(1);
        sc.apply_command(&SessionControlCommand::Pause);
        let changed = sc.apply_command(&SessionControlCommand::Pause);
        assert!(!changed);
        assert_eq!(sc.state, SessionState::Paused);
    }

    #[test]
    fn resume_transitions_paused_to_active() {
        let mut sc = SessionControl::new(1);
        sc.apply_command(&SessionControlCommand::Pause);
        let changed = sc.apply_command(&SessionControlCommand::Resume);
        assert!(changed);
        assert_eq!(sc.state, SessionState::Active);
    }

    #[test]
    fn resume_noop_when_already_active() {
        let mut sc = SessionControl::new(1);
        let changed = sc.apply_command(&SessionControlCommand::Resume);
        assert!(!changed);
        assert_eq!(sc.state, SessionState::Active);
    }

    #[test]
    fn set_group_changes_group_id_and_scope() {
        let mut sc = SessionControl::new(1);
        let changed = sc.apply_command(&SessionControlCommand::SetGroup { group_id: 2 });
        assert!(changed);
        assert_eq!(sc.group_id, 2);
        assert!(matches!(
            sc.routing_scope,
            RoutingScope::Group { group_id: 2, .. }
        ));
    }

    #[test]
    fn set_group_zero_resets_to_all_session() {
        let mut sc = SessionControl::with_group(1, 3);
        sc.apply_command(&SessionControlCommand::SetGroup { group_id: 0 });
        assert_eq!(sc.group_id, 0);
        assert_eq!(sc.routing_scope, RoutingScope::AllSession);
    }

    #[test]
    fn set_group_noop_when_same_group() {
        let mut sc = SessionControl::with_group(1, 2);
        let changed = sc.apply_command(&SessionControlCommand::SetGroup { group_id: 2 });
        assert!(!changed);
    }

    #[test]
    fn set_group_reports_change_when_scope_changes() {
        let mut sc = SessionControl::with_group(1, 2);
        sc.apply_command(&SessionControlCommand::BroadcastAll);
        let changed = sc.apply_command(&SessionControlCommand::SetGroup { group_id: 2 });
        assert!(changed);
        assert!(matches!(
            sc.routing_scope,
            RoutingScope::Group { group_id: 2, .. }
        ));
    }

    #[test]
    fn broadcast_all_sets_all_session_scope() {
        let mut sc = SessionControl::with_group(1, 2);
        let changed = sc.apply_command(&SessionControlCommand::BroadcastAll);
        assert!(changed);
        assert_eq!(sc.routing_scope, RoutingScope::AllSession);
    }

    #[test]
    fn broadcast_all_noop_when_already_all_session() {
        let mut sc = SessionControl::new(1);
        let changed = sc.apply_command(&SessionControlCommand::BroadcastAll);
        assert!(!changed);
    }

    #[test]
    fn set_error_and_clear_error() {
        let mut sc = SessionControl::new(1);
        sc.set_error();
        assert_eq!(sc.state, SessionState::Error);
        let cleared = sc.clear_error();
        assert!(cleared);
        assert_eq!(sc.state, SessionState::Active);
    }

    #[test]
    fn clear_error_noop_when_active() {
        let mut sc = SessionControl::new(1);
        let cleared = sc.clear_error();
        assert!(!cleared);
    }

    #[test]
    fn display_state_variants() {
        assert_eq!(SessionState::Active.to_string(), "Active");
        assert_eq!(SessionState::Paused.to_string(), "Paused");
        assert_eq!(SessionState::Error.to_string(), "Error");
    }

    #[test]
    fn resume_does_not_recover_error_state() {
        let mut sc = SessionControl::new(1);
        sc.set_error();
        // Resume only transitions Paused → Active, not Error → Active
        let changed = sc.apply_command(&SessionControlCommand::Resume);
        assert!(!changed);
        assert_eq!(sc.state, SessionState::Error);
    }
}
