//! Unified box-controller commands and automation state snapshots.

use serde::{Deserialize, Serialize};

/// Standardized automation mode shared across box-controller clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BoxControllerMode {
    /// Default automation state when a client is online and not otherwise
    /// constrained.
    #[default]
    Automatic,
    /// Automation is paused pending operator intervention.
    Paused,
    /// Client should return to camp behavior.
    Camp,
    /// Client should chase its configured leader or target.
    Chase,
    /// Client is in a manual / operator-driven mode.
    Manual,
}

/// Standard unified commands modeled after MQ2Boxr.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BoxControllerCommand {
    Pause,
    #[default]
    Unpause,
    Camp,
    Chase,
    Manual,
    BurnNow,
    RaidAssistNum {
        assist_num: u8,
    },
}

/// Automation state tracked for one character.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxControllerClientState {
    pub character_name: String,
    pub mode: BoxControllerMode,
    pub burn_requests: u32,
    #[serde(default)]
    pub raid_assist_num: Option<u8>,
    pub last_command: BoxControllerCommand,
}

impl BoxControllerClientState {
    #[must_use]
    pub fn new(character_name: String) -> Self {
        Self {
            character_name,
            mode: BoxControllerMode::Automatic,
            burn_requests: 0,
            raid_assist_num: None,
            last_command: BoxControllerCommand::default(),
        }
    }

    pub fn apply_command(&mut self, command: &BoxControllerCommand) {
        match command {
            BoxControllerCommand::Pause => {
                self.mode = BoxControllerMode::Paused;
            }
            BoxControllerCommand::Unpause => {
                self.mode = BoxControllerMode::Automatic;
            }
            BoxControllerCommand::Camp => {
                self.mode = BoxControllerMode::Camp;
            }
            BoxControllerCommand::Chase => {
                self.mode = BoxControllerMode::Chase;
            }
            BoxControllerCommand::Manual => {
                self.mode = BoxControllerMode::Manual;
            }
            BoxControllerCommand::BurnNow => {
                self.burn_requests = self.burn_requests.saturating_add(1);
            }
            BoxControllerCommand::RaidAssistNum { assist_num } => {
                self.raid_assist_num = Some(*assist_num);
            }
        }

        self.last_command = command.clone();
    }
}

/// Dashboard-facing view of one tracked automation client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxControllerClientSnapshot {
    pub character_name: String,
    pub node_name: String,
    pub mode: BoxControllerMode,
    pub burn_requests: u32,
    #[serde(default)]
    pub raid_assist_num: Option<u8>,
}

impl BoxControllerClientSnapshot {
    #[must_use]
    pub fn from_state(node_name: String, state: &BoxControllerClientState) -> Self {
        Self {
            character_name: state.character_name.clone(),
            node_name,
            mode: state.mode,
            burn_requests: state.burn_requests,
            raid_assist_num: state.raid_assist_num,
        }
    }
}

/// Global dashboard snapshot for the unified box-controller surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BoxControllerSnapshot {
    pub connected_clients: usize,
    pub relay_enabled: bool,
    #[serde(default)]
    pub last_command: Option<BoxControllerCommand>,
    pub clients: Vec<BoxControllerClientSnapshot>,
}
