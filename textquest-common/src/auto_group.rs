use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

fn default_retry_interval_secs() -> u64 {
    5
}

fn default_max_invite_retries() -> u8 {
    3
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoGroupSettings {
    #[serde(default)]
    pub groups: Vec<AutoGroupProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoGroupProfile {
    pub name: String,
    pub leader_name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_retry_interval_secs")]
    pub invite_retry_interval_secs: u64,
    #[serde(default = "default_max_invite_retries")]
    pub max_invite_retries: u8,
    #[serde(default)]
    pub completion_command: Option<String>,
    #[serde(default)]
    pub members: Vec<AutoGroupMember>,
}

impl Default for AutoGroupProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            leader_name: String::new(),
            enabled: false,
            invite_retry_interval_secs: default_retry_interval_secs(),
            max_invite_retries: default_max_invite_retries(),
            completion_command: None,
            members: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoGroupMember {
    pub name: String,
    #[serde(default)]
    pub role: AutoGroupRole,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoGroupRole {
    #[default]
    None,
    MainTank,
    MainAssist,
    Puller,
    MarkNpc,
    MasterLooter,
}

impl AutoGroupRole {
    #[must_use]
    pub fn grouproles_id(self) -> Option<u8> {
        match self {
            Self::None => None,
            Self::MainTank => Some(1),
            Self::MainAssist => Some(2),
            Self::Puller => Some(3),
            Self::MarkNpc => Some(4),
            Self::MasterLooter => Some(5),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutoGroupObservation {
    pub live_characters: BTreeSet<String>,
    pub leader_group_members: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoGroupCommandPhase {
    Invite,
    AcceptInvite,
    AssignRole,
    CompletionCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoGroupCommand {
    pub character_name: String,
    pub command: String,
    pub phase: AutoGroupCommandPhase,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AutoGroupProfileKey {
    index: usize,
    name: String,
    leader_name: String,
}

impl AutoGroupProfileKey {
    fn new(index: usize, profile: &AutoGroupProfile) -> Self {
        Self {
            index,
            name: profile.name.clone(),
            leader_name: normalize_name(&profile.leader_name),
        }
    }
}

#[derive(Debug, Default)]
struct AutoGroupProfileState {
    invite_attempts: BTreeMap<String, InviteAttemptState>,
    assigned_roles: BTreeSet<String>,
    completion_ran: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InviteAttemptState {
    attempts: u8,
    last_invite_at_secs: u64,
}

#[derive(Debug, Default)]
pub struct AutoGroupController {
    profiles: BTreeMap<AutoGroupProfileKey, AutoGroupProfileState>,
}

impl AutoGroupController {
    #[must_use]
    pub fn tick(
        &mut self,
        settings: &AutoGroupSettings,
        observation: &AutoGroupObservation,
        now_secs: u64,
    ) -> Vec<AutoGroupCommand> {
        let active_keys: BTreeSet<_> = settings
            .groups
            .iter()
            .enumerate()
            .filter(|(_, profile)| profile.enabled && !profile.leader_name.trim().is_empty())
            .map(|(index, profile)| AutoGroupProfileKey::new(index, profile))
            .collect();
        self.profiles.retain(|key, _| active_keys.contains(key));

        let live_characters: BTreeSet<_> = observation
            .live_characters
            .iter()
            .map(|name| normalize_name(name))
            .collect();
        let observed_groups: BTreeMap<_, _> = observation
            .leader_group_members
            .iter()
            .map(|(leader, members)| {
                (
                    normalize_name(leader),
                    members
                        .iter()
                        .map(|member| normalize_name(member))
                        .collect::<BTreeSet<_>>(),
                )
            })
            .collect();

        let mut commands = Vec::new();
        for (index, profile) in settings.groups.iter().enumerate() {
            if !profile.enabled || profile.leader_name.trim().is_empty() {
                continue;
            }

            let profile_key = AutoGroupProfileKey::new(index, profile);
            let profile_state = self.profiles.entry(profile_key).or_default();
            let leader_name = normalize_name(&profile.leader_name);
            if !live_characters.contains(&leader_name) {
                profile_state.invite_attempts.clear();
                profile_state.assigned_roles.clear();
                profile_state.completion_ran = false;
                continue;
            }

            let mut group_members = observed_groups
                .get(&leader_name)
                .cloned()
                .unwrap_or_else(|| BTreeSet::from([leader_name.clone()]));
            group_members.insert(leader_name.clone());

            profile_state
                .invite_attempts
                .retain(|member, _| !group_members.contains(member));

            let expected_members: BTreeSet<_> = profile
                .members
                .iter()
                .map(|member| normalize_name(&member.name))
                .collect();
            let group_is_complete = !expected_members.is_empty()
                && expected_members
                    .iter()
                    .all(|member| group_members.contains(member));
            if !group_is_complete {
                profile_state.assigned_roles.clear();
                profile_state.completion_ran = false;
            }

            let mut invite_issued = false;
            if !group_is_complete {
                for member in &profile.members {
                    let member_name = normalize_name(&member.name);
                    if member_name == leader_name || group_members.contains(&member_name) {
                        continue;
                    }

                    if !live_characters.contains(&member_name) {
                        break;
                    }

                    let should_invite = match profile_state.invite_attempts.get(&member_name) {
                        None => true,
                        Some(attempt) if attempt.attempts >= profile.max_invite_retries => false,
                        Some(attempt) => {
                            now_secs.saturating_sub(attempt.last_invite_at_secs)
                                >= profile.invite_retry_interval_secs
                        }
                    };
                    if should_invite {
                        let next_attempts = profile_state
                            .invite_attempts
                            .get(&member_name)
                            .map_or(1, |attempt| attempt.attempts.saturating_add(1));
                        profile_state.invite_attempts.insert(
                            member_name,
                            InviteAttemptState {
                                attempts: next_attempts,
                                last_invite_at_secs: now_secs,
                            },
                        );
                        commands.push(AutoGroupCommand {
                            character_name: profile.leader_name.clone(),
                            command: format!("/invite {}", member.name),
                            phase: AutoGroupCommandPhase::Invite,
                        });
                        commands.push(AutoGroupCommand {
                            character_name: member.name.clone(),
                            command: "/accept".to_string(),
                            phase: AutoGroupCommandPhase::AcceptInvite,
                        });
                        invite_issued = true;
                    }
                    break;
                }
            }

            if invite_issued || !group_is_complete {
                continue;
            }

            for member in &profile.members {
                let member_name = normalize_name(&member.name);
                let Some(role_id) = member.role.grouproles_id() else {
                    continue;
                };
                if member_name == leader_name || profile_state.assigned_roles.contains(&member_name)
                {
                    continue;
                }

                commands.push(AutoGroupCommand {
                    character_name: profile.leader_name.clone(),
                    command: format!("/grouproles set {} {}", member.name, role_id),
                    phase: AutoGroupCommandPhase::AssignRole,
                });
                profile_state.assigned_roles.insert(member_name);
            }

            if !profile_state.completion_ran {
                let completion_command = profile
                    .completion_command
                    .as_deref()
                    .map(str::trim)
                    .filter(|command| !command.is_empty());
                if let Some(completion_command) = completion_command {
                    commands.push(AutoGroupCommand {
                        character_name: profile.leader_name.clone(),
                        command: completion_command.to_string(),
                        phase: AutoGroupCommandPhase::CompletionCommand,
                    });
                }
                profile_state.completion_ran = true;
            }
        }

        commands
    }
}

fn normalize_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group_profile() -> AutoGroupProfile {
        AutoGroupProfile {
            name: "Alpha Team".into(),
            leader_name: "Alpha".into(),
            enabled: true,
            invite_retry_interval_secs: 5,
            max_invite_retries: 3,
            completion_command: Some("/say group ready".into()),
            members: vec![
                AutoGroupMember {
                    name: "Alpha".into(),
                    role: AutoGroupRole::None,
                },
                AutoGroupMember {
                    name: "Bravo".into(),
                    role: AutoGroupRole::MainTank,
                },
                AutoGroupMember {
                    name: "Charlie".into(),
                    role: AutoGroupRole::MainAssist,
                },
            ],
        }
    }

    fn base_observation() -> AutoGroupObservation {
        AutoGroupObservation {
            live_characters: ["Alpha", "Bravo", "Charlie"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            leader_group_members: BTreeMap::from([(
                String::from("Alpha"),
                vec![String::from("Alpha")],
            )]),
        }
    }

    #[test]
    fn tick_invites_missing_members_in_configured_order() {
        let settings = AutoGroupSettings {
            groups: vec![group_profile()],
        };
        let observation = base_observation();
        let mut controller = AutoGroupController::default();

        let commands = controller.tick(&settings, &observation, 0);

        assert_eq!(
            commands,
            vec![
                AutoGroupCommand {
                    character_name: "Alpha".into(),
                    command: "/invite Bravo".into(),
                    phase: AutoGroupCommandPhase::Invite,
                },
                AutoGroupCommand {
                    character_name: "Bravo".into(),
                    command: "/accept".into(),
                    phase: AutoGroupCommandPhase::AcceptInvite,
                },
            ]
        );
    }

    #[test]
    fn tick_retries_missing_member_after_retry_interval() {
        let settings = AutoGroupSettings {
            groups: vec![group_profile()],
        };
        let observation = base_observation();
        let mut controller = AutoGroupController::default();

        let first_commands = controller.tick(&settings, &observation, 0);
        assert_eq!(first_commands.len(), 2);

        let early_retry = controller.tick(&settings, &observation, 3);
        assert!(
            early_retry.is_empty(),
            "retry should wait for the configured interval"
        );

        let second_commands = controller.tick(&settings, &observation, 5);
        assert_eq!(
            second_commands,
            vec![
                AutoGroupCommand {
                    character_name: "Alpha".into(),
                    command: "/invite Bravo".into(),
                    phase: AutoGroupCommandPhase::Invite,
                },
                AutoGroupCommand {
                    character_name: "Bravo".into(),
                    command: "/accept".into(),
                    phase: AutoGroupCommandPhase::AcceptInvite,
                },
            ]
        );
    }

    #[test]
    fn tick_assigns_roles_and_runs_completion_once_group_is_full() {
        let settings = AutoGroupSettings {
            groups: vec![group_profile()],
        };
        let observation = AutoGroupObservation {
            live_characters: ["Alpha", "Bravo", "Charlie"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            leader_group_members: BTreeMap::from([(
                String::from("Alpha"),
                vec![
                    String::from("Alpha"),
                    String::from("Bravo"),
                    String::from("Charlie"),
                ],
            )]),
        };
        let mut controller = AutoGroupController::default();

        let commands = controller.tick(&settings, &observation, 10);

        assert_eq!(
            commands,
            vec![
                AutoGroupCommand {
                    character_name: "Alpha".into(),
                    command: "/grouproles set Bravo 1".into(),
                    phase: AutoGroupCommandPhase::AssignRole,
                },
                AutoGroupCommand {
                    character_name: "Alpha".into(),
                    command: "/grouproles set Charlie 2".into(),
                    phase: AutoGroupCommandPhase::AssignRole,
                },
                AutoGroupCommand {
                    character_name: "Alpha".into(),
                    command: "/say group ready".into(),
                    phase: AutoGroupCommandPhase::CompletionCommand,
                },
            ]
        );

        let repeated = controller.tick(&settings, &observation, 11);
        assert!(
            repeated.is_empty(),
            "once roles and completion command have run, later ticks should stay quiet"
        );
    }

    #[test]
    fn tick_keeps_duplicate_profile_names_independent() {
        let mut first_profile = group_profile();
        first_profile.completion_command = Some("/say first ready".into());
        first_profile.members = vec![
            AutoGroupMember {
                name: "Alpha".into(),
                role: AutoGroupRole::None,
            },
            AutoGroupMember {
                name: "Bravo".into(),
                role: AutoGroupRole::MainTank,
            },
        ];

        let mut second_profile = group_profile();
        second_profile.completion_command = Some("/say second ready".into());
        second_profile.members = vec![
            AutoGroupMember {
                name: "Alpha".into(),
                role: AutoGroupRole::None,
            },
            AutoGroupMember {
                name: "Charlie".into(),
                role: AutoGroupRole::MainAssist,
            },
        ];

        let settings = AutoGroupSettings {
            groups: vec![first_profile, second_profile],
        };
        let observation = AutoGroupObservation {
            live_characters: ["Alpha", "Bravo", "Charlie"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            leader_group_members: BTreeMap::from([(
                String::from("Alpha"),
                vec![
                    String::from("Alpha"),
                    String::from("Bravo"),
                    String::from("Charlie"),
                ],
            )]),
        };
        let mut controller = AutoGroupController::default();

        let commands = controller.tick(&settings, &observation, 10);

        assert_eq!(
            commands,
            vec![
                AutoGroupCommand {
                    character_name: "Alpha".into(),
                    command: "/grouproles set Bravo 1".into(),
                    phase: AutoGroupCommandPhase::AssignRole,
                },
                AutoGroupCommand {
                    character_name: "Alpha".into(),
                    command: "/say first ready".into(),
                    phase: AutoGroupCommandPhase::CompletionCommand,
                },
                AutoGroupCommand {
                    character_name: "Alpha".into(),
                    command: "/grouproles set Charlie 2".into(),
                    phase: AutoGroupCommandPhase::AssignRole,
                },
                AutoGroupCommand {
                    character_name: "Alpha".into(),
                    command: "/say second ready".into(),
                    phase: AutoGroupCommandPhase::CompletionCommand,
                },
            ]
        );
    }
}
