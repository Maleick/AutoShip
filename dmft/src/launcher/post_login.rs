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

pub enum PostLoginEvent {
    GroupJoined,
    BuffsApplied,
    CampReached,
}

impl PostLoginSequencer {
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
                if !self.camp_waypoints.is_empty() {
                    Some(Command::NavigateTo {
                        waypoints: self.camp_waypoints.clone(),
                    })
                } else {
                    Some(Command::ReportReady)
                }
            }
            PostLoginPhase::NavigatingToCamp => {
                // Waiting for navigation to complete (check nav_status)
                None // nav system handles this autonomously
            }
            PostLoginPhase::Ready => None,
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
                    tracing::info!(
                        client_id = self.client_id,
                        "Buffs done, navigating to camp"
                    );
                    self.phase = PostLoginPhase::NavigatingToCamp;
                }
            }
            PostLoginEvent::CampReached => {
                tracing::info!(client_id = self.client_id, "At camp — ready");
                self.phase = PostLoginPhase::Ready;
            }
        }
    }

    pub fn phase(&self) -> &PostLoginPhase {
        &self.phase
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.phase, PostLoginPhase::Ready)
    }

    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }
}
