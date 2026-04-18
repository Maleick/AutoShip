//! Session and group control model for the orchestrator.
//!
//! `SessionControl` tracks per-session routing scope, group membership, and
//! active/paused state. Commands arrive via the IPC layer as
//! `SessionControlCommand` variants and are processed by `apply_command`.
//!
//! This module also defines the stable admin-session snapshot returned by the
//! web admin API. The snapshot stays focused on orchestrator-managed metadata
//! and keeps Windows-only identifiers optional so Linux/macOS builds can still
//! emit an explicit inventory.

use std::collections::HashMap;
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

/// Stable lifecycle label exposed by `/api/admin/sessions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdminSessionLifecycle {
    Active,
    Paused,
    Error,
}

impl From<SessionState> for AdminSessionLifecycle {
    fn from(value: SessionState) -> Self {
        match value {
            SessionState::Active => Self::Active,
            SessionState::Paused => Self::Paused,
            SessionState::Error => Self::Error,
        }
    }
}

/// Stable routing-scope kind exposed by `/api/admin/sessions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdminRoutingScopeKind {
    OneToon,
    Group,
    AllSession,
}

/// Admin-safe routing summary for one managed session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AdminRoutingScopeSnapshot {
    pub kind: AdminRoutingScopeKind,
    pub label: String,
    pub group_id: Option<u8>,
    pub toon_name: Option<String>,
}

impl From<&RoutingScope> for AdminRoutingScopeSnapshot {
    fn from(value: &RoutingScope) -> Self {
        match value {
            RoutingScope::OneToon { name } => Self {
                kind: AdminRoutingScopeKind::OneToon,
                label: value.label(),
                group_id: None,
                toon_name: Some(name.clone()),
            },
            RoutingScope::Group { group_id, .. } => Self {
                kind: AdminRoutingScopeKind::Group,
                label: value.label(),
                group_id: Some(*group_id),
                toon_name: None,
            },
            RoutingScope::AllSession => Self {
                kind: AdminRoutingScopeKind::AllSession,
                label: value.label(),
                group_id: None,
                toon_name: None,
            },
        }
    }
}

/// Stable admin snapshot for one orchestrator-managed session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AdminSessionSnapshot {
    /// Stable orchestrator session identifier. Today this matches the process
    /// ID used to register the session with the orchestrator.
    pub session_id: u32,
    /// Optional character name associated with the session. This is filled once
    /// launcher/login metadata is available; non-Windows test builds may leave
    /// it unset.
    pub character_name: Option<String>,
    /// Optional class name associated with the session. This is populated from
    /// launcher metadata when available and may remain unset on non-Windows
    /// builds.
    pub class_name: Option<String>,
    /// Current group assignment tracked by the orchestrator (`0` = ungrouped /
    /// broadcast-all).
    pub group_id: u8,
    pub routing_scope: AdminRoutingScopeSnapshot,
    pub lifecycle_state: AdminSessionLifecycle,
}

/// Build a deterministic admin inventory from the orchestrator's per-session
/// control state and known identifying metadata.
#[must_use]
pub fn build_admin_session_inventory(
    session_controls: &HashMap<u32, SessionControl>,
    client_names: &HashMap<u32, String>,
    client_class_names: &HashMap<u32, String>,
) -> Vec<AdminSessionSnapshot> {
    let mut snapshots: Vec<_> = session_controls
        .values()
        .map(|control| AdminSessionSnapshot {
            session_id: control.session_id,
            character_name: client_names.get(&control.session_id).cloned(),
            class_name: client_class_names.get(&control.session_id).cloned(),
            group_id: control.group_id,
            routing_scope: AdminRoutingScopeSnapshot::from(&control.routing_scope),
            lifecycle_state: AdminSessionLifecycle::from(control.state),
        })
        .collect();
    snapshots.sort_by_key(|snapshot| snapshot.session_id);
    snapshots
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
    use std::collections::HashMap;
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

    #[test]
    fn admin_inventory_builder_reports_session_metadata_and_scope() {
        let mut controls = HashMap::new();
        let mut control = SessionControl::with_group(4242, 2);
        control.apply_command(&SessionControlCommand::Pause);
        controls.insert(4242, control);

        let mut client_names = HashMap::new();
        client_names.insert(4242, String::from("Cleric42"));

        let mut client_class_names = HashMap::new();
        client_class_names.insert(4242, String::from("Cleric"));

        let snapshots =
            build_admin_session_inventory(&controls, &client_names, &client_class_names);

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].session_id, 4242);
        assert_eq!(snapshots[0].character_name.as_deref(), Some("Cleric42"));
        assert_eq!(snapshots[0].class_name.as_deref(), Some("Cleric"));
        assert_eq!(snapshots[0].group_id, 2);
        assert_eq!(snapshots[0].lifecycle_state, AdminSessionLifecycle::Paused);
        assert_eq!(
            snapshots[0].routing_scope.kind,
            AdminRoutingScopeKind::Group
        );
        assert_eq!(snapshots[0].routing_scope.group_id, Some(2));
        assert_eq!(snapshots[0].routing_scope.label, "G2");
    }

    #[test]
    fn admin_inventory_builder_sorts_sessions_and_keeps_optional_metadata_null() {
        let controls = HashMap::from([
            (9, SessionControl::new(9)),
            (2, SessionControl::with_group(2, 1)),
        ]);

        let snapshots = build_admin_session_inventory(&controls, &HashMap::new(), &HashMap::new());

        assert_eq!(
            snapshots
                .iter()
                .map(|snapshot| snapshot.session_id)
                .collect::<Vec<_>>(),
            vec![2, 9]
        );
        assert_eq!(snapshots[0].character_name, None);
        assert_eq!(snapshots[0].class_name, None);
        assert_eq!(
            snapshots[0].routing_scope.kind,
            AdminRoutingScopeKind::Group
        );
        assert_eq!(
            snapshots[1].routing_scope.kind,
            AdminRoutingScopeKind::AllSession
        );
    }
}
