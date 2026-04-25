use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::persistence::{load_json_config, save_json_config};
use crate::{ipc::AutoRezConfig, window_title::default_window_title_format};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RotationEntry {
    pub id: String,
    pub name: String,
    pub priority: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ClassParams {
    pub ch_chain_timing_ms: Option<u32>,
    pub dot_overlap_pct: Option<u8>,
    pub burn_at_hp_pct: Option<u8>,
    pub slow_at_hp_pct: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewardPreference {
    ByName { reward_name: String },
    ByPosition { reward_position: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum HumanReadableRewardPreference {
    ByName { reward_name: String },
    ByPosition { reward_position: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum BinaryRewardPreference {
    ByName { reward_name: String },
    ByPosition { reward_position: usize },
}

impl Serialize for RewardPreference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let helper = match self {
                Self::ByName { reward_name } => HumanReadableRewardPreference::ByName {
                    reward_name: reward_name.clone(),
                },
                Self::ByPosition { reward_position } => HumanReadableRewardPreference::ByPosition {
                    reward_position: *reward_position,
                },
            };
            helper.serialize(serializer)
        } else {
            let helper = match self {
                Self::ByName { reward_name } => BinaryRewardPreference::ByName {
                    reward_name: reward_name.clone(),
                },
                Self::ByPosition { reward_position } => BinaryRewardPreference::ByPosition {
                    reward_position: *reward_position,
                },
            };
            helper.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for RewardPreference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            match HumanReadableRewardPreference::deserialize(deserializer)? {
                HumanReadableRewardPreference::ByName { reward_name } => {
                    Ok(Self::ByName { reward_name })
                }
                HumanReadableRewardPreference::ByPosition { reward_position } => {
                    Ok(Self::ByPosition { reward_position })
                }
            }
        } else {
            match BinaryRewardPreference::deserialize(deserializer)? {
                BinaryRewardPreference::ByName { reward_name } => Ok(Self::ByName { reward_name }),
                BinaryRewardPreference::ByPosition { reward_position } => {
                    Ok(Self::ByPosition { reward_position })
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRewardPreference {
    /// Case-insensitive task window title matcher.
    /// `"*"` applies as the default fallback rule.
    pub task_matcher: String,
    pub preference: RewardPreference,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RewardAutomationConfig {
    #[serde(default)]
    pub rules: Vec<TaskRewardPreference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TributeAlertState {
    Ok,
    Expiring,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TributePreferences {
    pub auto_activate: bool,
    pub warning_threshold_secs: u64,
    #[serde(default)]
    pub preferred_tributes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TributeStatus {
    pub active: bool,
    pub remaining_secs: u64,
    pub point_balance: u32,
    #[serde(default)]
    pub active_tributes: Vec<String>,
    pub alert_state: TributeAlertState,
}

impl Default for TributeStatus {
    fn default() -> Self {
        Self {
            active: false,
            remaining_secs: 0,
            point_balance: 0,
            active_tributes: Vec::new(),
            alert_state: TributeAlertState::Expired,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ImproveAutoPromoteConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,
}

fn default_min_confidence() -> f32 {
    0.85
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterConfig {
    pub character_name: String,
    pub class: String,
    pub role: String,
    pub heal_at_pct: u8,
    pub mana_sit_pct: u8,
    pub nuke_at_pct: u8,
    pub rotation: Vec<RotationEntry>,
    pub class_params: ClassParams,
    #[serde(default)]
    pub auto_rez: AutoRezConfig,
    pub group_override: bool,
    pub group_name: Option<String>,
    #[serde(default = "default_window_title_format")]
    pub window_title_format: String,
    #[serde(default)]
    pub reward_automation: RewardAutomationConfig,
    #[serde(default)]
    pub tribute_preferences: TributePreferences,
    #[serde(default)]
    pub tribute_status: TributeStatus,
    #[serde(default)]
    pub improve_auto_promote: ImproveAutoPromoteConfig,
}

pub type CharacterConfigMap = HashMap<String, CharacterConfig>;

fn normalize_matcher(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

pub fn load_character_configs(path: &Path) -> Result<CharacterConfigMap> {
    let configs: CharacterConfigMap = load_json_config(path)
        .with_context(|| format!("failed to load character configs: {}", path.display()))?;
    Ok(configs)
}

pub fn save_character_configs(path: &Path, configs: &CharacterConfigMap) -> Result<()> {
    save_json_config(path, configs)
        .with_context(|| format!("failed to save character configs: {}", path.display()))
}

pub fn reward_preference_for_task<'a>(
    rules: &'a [TaskRewardPreference],
    task_name: &str,
) -> Option<&'a RewardPreference> {
    let normalized_task = normalize_matcher(task_name);

    rules
        .iter()
        .find(|rule| normalize_matcher(&rule.task_matcher) == normalized_task)
        .or_else(|| {
            rules.iter().find(|rule| {
                let matcher = normalize_matcher(&rule.task_matcher);
                matcher.is_empty() || matcher == "*"
            })
        })
        .map(|rule| &rule.preference)
}

pub fn resolve_reward_index(
    task_name: &str,
    reward_names: &[String],
    config: &RewardAutomationConfig,
) -> Option<usize> {
    if reward_names.is_empty() {
        return None;
    }

    let preferred = reward_preference_for_task(&config.rules, task_name);
    let selected = match preferred {
        Some(RewardPreference::ByName { reward_name }) => reward_names
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(reward_name))
            .unwrap_or(0),
        Some(RewardPreference::ByPosition { reward_position }) => {
            // Fallback to the first reward when the configured position isn't
            // present in the current reward list. Matches the semantic used
            // by ByName (unwrap_or(0)) and the `None` arm, rather than silently
            // claiming the last tab when the UI layout changes.
            let idx = reward_position.saturating_sub(1);
            if idx < reward_names.len() { idx } else { 0 }
        }
        None => 0,
    };

    Some(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> CharacterConfig {
        CharacterConfig {
            character_name: "Alpha".into(),
            class: "Wizard".into(),
            role: "DPS".into(),
            heal_at_pct: 50,
            mana_sit_pct: 20,
            nuke_at_pct: 90,
            rotation: vec![],
            class_params: ClassParams::default(),
            auto_rez: AutoRezConfig::default(),
            group_override: false,
            group_name: None,
            window_title_format: default_window_title_format(),
            reward_automation: RewardAutomationConfig::default(),
            tribute_preferences: TributePreferences::default(),
            tribute_status: TributeStatus::default(),
            improve_auto_promote: ImproveAutoPromoteConfig::default(),
        }
    }

    #[test]
    fn reward_preference_prefers_exact_task_match() {
        let rules = vec![
            TaskRewardPreference {
                task_matcher: "*".into(),
                preference: RewardPreference::ByPosition { reward_position: 2 },
            },
            TaskRewardPreference {
                task_matcher: "Cleansing the Nest".into(),
                preference: RewardPreference::ByName {
                    reward_name: "Shimmering Crystal".into(),
                },
            },
        ];

        let matched = reward_preference_for_task(&rules, "cleansing the nest")
            .expect("task should match exact rule");
        assert_eq!(
            matched,
            &RewardPreference::ByName {
                reward_name: "Shimmering Crystal".into()
            }
        );
    }

    #[test]
    fn resolve_reward_index_matches_name_case_insensitively() {
        let config = RewardAutomationConfig {
            rules: vec![TaskRewardPreference {
                task_matcher: "Expedition".into(),
                preference: RewardPreference::ByName {
                    reward_name: "glowing orb".into(),
                },
            }],
        };
        let rewards = vec![
            "Minor Potion".to_string(),
            "Glowing Orb".to_string(),
            "Ancient Coin".to_string(),
        ];

        assert_eq!(
            resolve_reward_index("expedition", &rewards, &config),
            Some(1)
        );
    }

    #[test]
    fn resolve_reward_index_uses_one_based_position() {
        let config = RewardAutomationConfig {
            rules: vec![TaskRewardPreference {
                task_matcher: "Mission".into(),
                preference: RewardPreference::ByPosition { reward_position: 3 },
            }],
        };
        let rewards = vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
        ];

        assert_eq!(resolve_reward_index("Mission", &rewards, &config), Some(2));
    }

    #[test]
    fn resolve_reward_index_falls_back_to_first_reward() {
        let config = RewardAutomationConfig {
            rules: vec![TaskRewardPreference {
                task_matcher: "*".into(),
                preference: RewardPreference::ByName {
                    reward_name: "Missing Reward".into(),
                },
            }],
        };
        let rewards = vec!["A".to_string(), "B".to_string()];

        assert_eq!(resolve_reward_index("Anything", &rewards, &config), Some(0));
    }

    #[test]
    fn resolve_reward_index_by_position_out_of_range_falls_back_to_first() {
        // Previous behavior clamped to the last available tab, which could
        // silently claim the wrong reward after UI layout changes. Fallback
        // should match ByName's unwrap_or(0) semantic.
        let config = RewardAutomationConfig {
            rules: vec![TaskRewardPreference {
                task_matcher: "*".into(),
                preference: RewardPreference::ByPosition { reward_position: 9 },
            }],
        };
        let rewards = vec!["A".to_string(), "B".to_string()];

        assert_eq!(resolve_reward_index("Anything", &rewards, &config), Some(0));
    }

    #[test]
    fn reward_preference_json_shape_matches_web_config_contract() {
        let pref = RewardPreference::ByPosition { reward_position: 2 };

        let encoded = serde_json::to_string(&pref).expect("serialize reward preference");

        assert_eq!(encoded, r#"{"kind":"by_position","reward_position":2}"#);
        let decoded: RewardPreference =
            serde_json::from_str(&encoded).expect("deserialize reward preference");
        assert_eq!(decoded, pref);
    }

    #[test]
    fn save_and_load_character_configs_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("character-configs.json");

        let mut configs = HashMap::new();
        configs.insert("Alpha".into(), sample_config());

        save_character_configs(&path, &configs).expect("save should succeed");
        let loaded = load_character_configs(&path).expect("load should succeed");

        assert_eq!(loaded, configs);
    }

    #[test]
    fn load_character_configs_returns_empty_for_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("missing.json");

        let loaded = load_character_configs(&path).expect("load should succeed");
        assert!(loaded.is_empty());
    }

    #[test]
    fn reward_preference_for_task_returns_none_when_rules_empty() {
        assert!(reward_preference_for_task(&[], "Any Task").is_none());
    }

    #[test]
    fn reward_preference_for_task_uses_wildcard_fallback_when_no_exact_match() {
        let rules = vec![
            TaskRewardPreference {
                task_matcher: "*".into(),
                preference: RewardPreference::ByPosition { reward_position: 1 },
            },
            TaskRewardPreference {
                task_matcher: "Specific Task".into(),
                preference: RewardPreference::ByPosition { reward_position: 2 },
            },
        ];

        let pref = reward_preference_for_task(&rules, "Other Task").expect("wildcard should match");
        assert_eq!(pref, &RewardPreference::ByPosition { reward_position: 1 });
    }

    #[test]
    fn reward_preference_for_task_prefers_exact_match_over_wildcard() {
        let rules = vec![
            TaskRewardPreference {
                task_matcher: "*".into(),
                preference: RewardPreference::ByPosition { reward_position: 1 },
            },
            TaskRewardPreference {
                task_matcher: "Special Mission".into(),
                preference: RewardPreference::ByPosition { reward_position: 3 },
            },
        ];

        let pref =
            reward_preference_for_task(&rules, "special mission").expect("exact match should win");
        assert_eq!(pref, &RewardPreference::ByPosition { reward_position: 3 });
    }

    #[test]
    fn resolve_reward_index_returns_none_when_rewards_empty() {
        let config = RewardAutomationConfig {
            rules: vec![TaskRewardPreference {
                task_matcher: "*".into(),
                preference: RewardPreference::ByPosition { reward_position: 1 },
            }],
        };
        assert_eq!(resolve_reward_index("Any Task", &[], &config), None);
    }

    #[test]
    fn resolve_reward_index_returns_first_when_no_rule_matches() {
        let config = RewardAutomationConfig { rules: vec![] };
        let rewards = vec!["A".to_string(), "B".to_string()];
        assert_eq!(resolve_reward_index("Any Task", &rewards, &config), Some(0));
    }

    #[test]
    fn resolve_reward_index_by_position_one_based_at_boundary() {
        let config = RewardAutomationConfig {
            rules: vec![TaskRewardPreference {
                task_matcher: "Quest".into(),
                preference: RewardPreference::ByPosition { reward_position: 1 },
            }],
        };
        let rewards = vec!["First".to_string(), "Second".to_string()];
        // Position 1 → index 0
        assert_eq!(resolve_reward_index("Quest", &rewards, &config), Some(0));
    }

    #[test]
    fn tribute_status_default_is_expired_and_inactive() {
        let status = TributeStatus::default();
        assert!(!status.active);
        assert_eq!(status.remaining_secs, 0);
        assert_eq!(status.point_balance, 0);
        assert!(status.active_tributes.is_empty());
        assert_eq!(status.alert_state, TributeAlertState::Expired);
    }

    #[test]
    fn reward_preference_by_name_json_round_trip() {
        let pref = RewardPreference::ByName {
            reward_name: "Shiny Gem".into(),
        };
        let encoded = serde_json::to_string(&pref).expect("serialize");
        let decoded: RewardPreference = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, pref);
    }
}
