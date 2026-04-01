use dmft_common::ipc::Command;
use dmft_common::types::{ClientId, GameState};

use crate::client::session::PostLoginPhase;
use std::time::Instant;

/// Sequences post-login actions: group join -> buff -> navigate to camp -> ready.
pub struct PostLoginSequencer {
    client_id: ClientId,
    group_id: u32,
    phase: PostLoginPhase,
    started_at: Instant,
    camp_waypoints: Vec<dmft_common::nav::Waypoint>,
}

/// Events that advance the post-login sequencer.
pub enum PostLoginEvent {
    /// Client successfully joined the designated group.
    GroupJoined,
    /// All required buffs have been applied.
    BuffsApplied,
    /// Client has arrived at the camp location.
    CampReached,
}

impl PostLoginSequencer {
    /// Creates a new sequencer for the given client, group, and camp route.
    #[must_use]
    pub fn new(
        client_id: ClientId,
        group_id: u32,
        camp_waypoints: Vec<dmft_common::nav::Waypoint>,
    ) -> Self {
        Self {
            client_id,
            group_id,
            phase: PostLoginPhase::NotStarted,
            started_at: Instant::now(),
            camp_waypoints,
        }
    }

    /// Get the next command to send based on current phase and game state.
    #[must_use]
    pub fn next_command(&self, _state: &GameState) -> Option<Command> {
        match self.phase {
            PostLoginPhase::NotStarted => Some(Command::JoinGroup {
                group_id: self.group_id,
            }),
            PostLoginPhase::JoiningGroup => {
                // Waiting for group join confirmation — check state for group membership
                // For now, immediately proceed to buffing
                Some(Command::ApplyBuffs)
            }
            PostLoginPhase::Buffing => {
                // Waiting for buffs to be applied
                if self.camp_waypoints.is_empty() {
                    Some(Command::ReportReady)
                } else {
                    Some(Command::NavigateTo {
                        waypoints: self.camp_waypoints.clone(),
                    })
                }
            }
            PostLoginPhase::NavigatingToCamp => {
                // Waiting for navigation to complete (check nav_status)
                None // nav system handles this autonomously
            }
            PostLoginPhase::Ready => None,
        }
    }

    /// Mark that the current phase's command was dispatched.
    /// Advances `NotStarted` -> `JoiningGroup` after the `JoinGroup` command is sent.
    pub fn mark_dispatched(&mut self) {
        if matches!(self.phase, PostLoginPhase::NotStarted) {
            self.phase = PostLoginPhase::JoiningGroup;
        }
    }

    /// Advance the sequencer based on an event.
    pub fn advance(&mut self, event: PostLoginEvent) {
        match event {
            PostLoginEvent::GroupJoined => {
                tracing::info!(client_id = self.client_id, "Joined group, starting buffs");
                self.phase = PostLoginPhase::Buffing;
            }
            PostLoginEvent::BuffsApplied => {
                if self.camp_waypoints.is_empty() {
                    tracing::info!(client_id = self.client_id, "Buffs done, no camp — ready");
                    self.phase = PostLoginPhase::Ready;
                } else {
                    tracing::info!(client_id = self.client_id, "Buffs done, navigating to camp");
                    self.phase = PostLoginPhase::NavigatingToCamp;
                }
            }
            PostLoginEvent::CampReached => {
                tracing::info!(client_id = self.client_id, "At camp — ready");
                self.phase = PostLoginPhase::Ready;
            }
        }
    }

    /// The current post-login phase.
    #[must_use]
    pub fn phase(&self) -> &PostLoginPhase {
        &self.phase
    }

    /// Whether the post-login sequence is complete and the client is ready.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self.phase, PostLoginPhase::Ready)
    }

    /// The client ID this sequencer manages.
    #[must_use]
    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    /// Time elapsed since the sequencer was created.
    #[must_use]
    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }
}

/// Generate `/invite <name>` commands for forming groups.
/// `leader_pid` is the PID of the group leader's EQ client.
/// `member_names` are the character names to invite.
/// Returns a list of `SlashCommand` to send to the leader's DLL.
#[must_use]
pub fn group_invite_commands(member_names: &[&str]) -> Vec<Command> {
    member_names
        .iter()
        .map(|name| Command::SlashCommand {
            command: format!("/invite {name}"),
        })
        .collect()
}

/// Generate `/accept` command for group members to accept invites.
#[must_use]
pub fn group_accept_command() -> Command {
    Command::SlashCommand {
        command: "/accept".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dmft_common::nav::Waypoint;

    fn test_game_state() -> GameState {
        GameState {
            client_id: 1,
            local_player: None,
            target: None,
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: dmft_common::nav::NavStatus::Idle,
            combat_status: dmft_common::combat::CombatStatus::Idle,
            zone_short_name: String::new(),
            zone_long_name: String::new(),
        }
    }

    #[test]
    fn initial_phase_is_not_started() {
        let seq = PostLoginSequencer::new(1, 1, vec![]);
        assert!(matches!(seq.phase(), PostLoginPhase::NotStarted));
        assert!(!seq.is_ready());
    }

    #[test]
    fn not_started_generates_join_group_command() {
        let seq = PostLoginSequencer::new(1, 42, vec![]);
        let state = test_game_state();
        let cmd = seq.next_command(&state);
        match cmd {
            Some(Command::JoinGroup { group_id }) => assert_eq!(group_id, 42),
            other => panic!("expected JoinGroup, got {other:?}"),
        }
    }

    #[test]
    fn advance_group_joined_transitions_to_buffing() {
        let mut seq = PostLoginSequencer::new(1, 1, vec![]);
        seq.advance(PostLoginEvent::GroupJoined);
        assert!(matches!(seq.phase(), PostLoginPhase::Buffing));
    }

    #[test]
    fn buffing_with_no_waypoints_generates_report_ready() {
        let mut seq = PostLoginSequencer::new(1, 1, vec![]);
        seq.advance(PostLoginEvent::GroupJoined);
        let state = test_game_state();
        let cmd = seq.next_command(&state);
        assert!(matches!(cmd, Some(Command::ReportReady)));
    }

    #[test]
    fn buffing_with_waypoints_generates_navigate_to() {
        let waypoints = vec![Waypoint::new(100.0, 200.0, 0.0)];
        let mut seq = PostLoginSequencer::new(1, 1, waypoints);
        seq.advance(PostLoginEvent::GroupJoined);
        let state = test_game_state();
        let cmd = seq.next_command(&state);
        assert!(matches!(cmd, Some(Command::NavigateTo { .. })));
    }

    #[test]
    fn buffs_applied_without_waypoints_goes_to_ready() {
        let mut seq = PostLoginSequencer::new(1, 1, vec![]);
        seq.advance(PostLoginEvent::GroupJoined);
        seq.advance(PostLoginEvent::BuffsApplied);
        assert!(matches!(seq.phase(), PostLoginPhase::Ready));
        assert!(seq.is_ready());
    }

    #[test]
    fn buffs_applied_with_waypoints_goes_to_navigating() {
        let waypoints = vec![Waypoint::new(100.0, 200.0, 0.0)];
        let mut seq = PostLoginSequencer::new(1, 1, waypoints);
        seq.advance(PostLoginEvent::GroupJoined);
        seq.advance(PostLoginEvent::BuffsApplied);
        assert!(matches!(seq.phase(), PostLoginPhase::NavigatingToCamp));
        assert!(!seq.is_ready());
    }

    #[test]
    fn camp_reached_transitions_to_ready() {
        let waypoints = vec![Waypoint::new(100.0, 200.0, 0.0)];
        let mut seq = PostLoginSequencer::new(1, 1, waypoints);
        seq.advance(PostLoginEvent::GroupJoined);
        seq.advance(PostLoginEvent::BuffsApplied);
        seq.advance(PostLoginEvent::CampReached);
        assert!(matches!(seq.phase(), PostLoginPhase::Ready));
        assert!(seq.is_ready());
    }

    #[test]
    fn navigating_phase_generates_no_command() {
        let waypoints = vec![Waypoint::new(100.0, 200.0, 0.0)];
        let mut seq = PostLoginSequencer::new(1, 1, waypoints);
        seq.advance(PostLoginEvent::GroupJoined);
        seq.advance(PostLoginEvent::BuffsApplied);
        let state = test_game_state();
        let cmd = seq.next_command(&state);
        assert!(cmd.is_none(), "nav system handles movement autonomously");
    }

    #[test]
    fn ready_phase_generates_no_command() {
        let mut seq = PostLoginSequencer::new(1, 1, vec![]);
        seq.advance(PostLoginEvent::GroupJoined);
        seq.advance(PostLoginEvent::BuffsApplied);
        let state = test_game_state();
        let cmd = seq.next_command(&state);
        assert!(cmd.is_none());
    }

    #[test]
    fn client_id_accessor_returns_correct_id() {
        let seq = PostLoginSequencer::new(42, 1, vec![]);
        assert_eq!(seq.client_id(), 42);
    }

    #[test]
    fn elapsed_does_not_panic_after_creation() {
        let seq = PostLoginSequencer::new(1, 1, vec![]);
        // Ensure calling elapsed() immediately after creation does not panic.
        let _ = seq.elapsed();
    }

    #[test]
    fn mark_dispatched_advances_not_started_to_joining() {
        let mut seq = PostLoginSequencer::new(1, 1, vec![]);
        assert!(matches!(seq.phase(), PostLoginPhase::NotStarted));
        seq.mark_dispatched();
        assert!(matches!(seq.phase(), PostLoginPhase::JoiningGroup));
    }

    #[test]
    fn mark_dispatched_no_op_after_joining() {
        let mut seq = PostLoginSequencer::new(1, 1, vec![]);
        seq.mark_dispatched();
        assert!(matches!(seq.phase(), PostLoginPhase::JoiningGroup));
        seq.mark_dispatched(); // Should be a no-op
        assert!(matches!(seq.phase(), PostLoginPhase::JoiningGroup));
    }

    #[test]
    fn joining_group_generates_apply_buffs_command() {
        let mut seq = PostLoginSequencer::new(1, 1, vec![]);
        seq.mark_dispatched(); // -> JoiningGroup
        let state = test_game_state();
        let cmd = seq.next_command(&state);
        assert!(matches!(cmd, Some(Command::ApplyBuffs)));
    }

    #[test]
    fn full_lifecycle_no_waypoints() {
        let mut seq = PostLoginSequencer::new(1, 1, vec![]);
        assert!(!seq.is_ready());

        seq.mark_dispatched();
        seq.advance(PostLoginEvent::GroupJoined);
        seq.advance(PostLoginEvent::BuffsApplied);

        assert!(seq.is_ready());
        assert!(matches!(seq.phase(), PostLoginPhase::Ready));
    }

    #[test]
    fn full_lifecycle_with_waypoints() {
        let waypoints = vec![
            Waypoint::new(100.0, 200.0, 0.0),
            Waypoint::new(300.0, 400.0, 10.0),
        ];
        let mut seq = PostLoginSequencer::new(1, 1, waypoints);
        assert!(!seq.is_ready());

        seq.mark_dispatched();
        seq.advance(PostLoginEvent::GroupJoined);
        seq.advance(PostLoginEvent::BuffsApplied);
        assert!(!seq.is_ready()); // Navigating

        seq.advance(PostLoginEvent::CampReached);
        assert!(seq.is_ready());
    }

    #[test]
    fn group_invite_commands_empty() {
        let cmds = group_invite_commands(&[]);
        assert!(cmds.is_empty());
    }

    #[test]
    fn group_invite_commands_multiple() {
        let cmds = group_invite_commands(&["Alice", "Bob", "Charlie"]);
        assert_eq!(cmds.len(), 3);
        if let Command::SlashCommand { command } = &cmds[0] {
            assert_eq!(command, "/invite Alice");
        } else {
            panic!("Expected SlashCommand");
        }
        if let Command::SlashCommand { command } = &cmds[2] {
            assert_eq!(command, "/invite Charlie");
        } else {
            panic!("Expected SlashCommand");
        }
    }

    #[test]
    fn group_accept_command_format() {
        let cmd = group_accept_command();
        if let Command::SlashCommand { command } = cmd {
            assert_eq!(command, "/accept");
        } else {
            panic!("Expected SlashCommand");
        }
    }
}
