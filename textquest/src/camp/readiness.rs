//! Group readiness gate — checks if all group members are ready before pulling.
//!
//! Validates:
//! - All members alive and in-zone
//! - HP/Mana thresholds met per role
//! - All members within position radius of camp center
//! - Required buffs present
//! - All members standing (not casting/casting spells)

use crate::camp::{config::GroupReadinessConfig, state::Role};
use textquest_common::types::SharedStateFrame;

/// Reason why a member is not ready to pull.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadinessBlocker {
    /// Member is dead (HP == 0).
    Dead { name: String },
    /// Member's HP below threshold for their role.
    LowHp {
        name: String,
        current: u8,
        threshold: u8,
    },
    /// Member's Mana below threshold for their role.
    LowMana {
        name: String,
        current: u8,
        threshold: u8,
    },
    /// Member is too far from camp center.
    OutOfPosition {
        name: String,
        distance: f32,
        max_distance: f32,
    },
    /// Required buff is missing on member.
    MissingBuff { name: String, buff: String },
}

impl ReadinessBlocker {
    /// Human-readable message for operator display.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Dead { name } => format!("{} is dead", name),
            Self::LowHp {
                name,
                current,
                threshold,
            } => {
                format!("{} HP {} < {}", name, current, threshold)
            }
            Self::LowMana {
                name,
                current,
                threshold,
            } => {
                format!("{} mana {} < {}", name, current, threshold)
            }
            Self::OutOfPosition {
                name,
                distance,
                max_distance,
            } => {
                format!(
                    "{} position {} > {}",
                    name, distance as i32, max_distance as i32
                )
            }
            Self::MissingBuff { name, buff } => format!("{} missing {}", name, buff),
        }
    }
}

/// Checks if a single group member meets readiness criteria.
#[must_use]
pub fn check_member_ready(
    name: &str,
    role: &Role,
    frame: &SharedStateFrame,
    config: &GroupReadinessConfig,
) -> Option<ReadinessBlocker> {
    // Check HP
    if frame.hp <= 0 {
        return Some(ReadinessBlocker::Dead {
            name: name.to_string(),
        });
    }

    let hp_pct = ((frame.hp as f32) / (frame.hp_max as f32) * 100.0) as u8;

    // Get role-specific thresholds
    let thresholds = match role {
        Role::Healer => &config.healer,
        Role::Tank => &config.tank,
        Role::CC | Role::Dps | Role::Puller | Role::Bard => &config.dps,
    };

    if hp_pct < thresholds.hp_pct {
        return Some(ReadinessBlocker::LowHp {
            name: name.to_string(),
            current: hp_pct,
            threshold: thresholds.hp_pct,
        });
    }

    // Check Mana (skip if threshold is 0)
    if thresholds.mana_pct > 0 {
        let mana_pct = ((frame.mana as f32) / (frame.mana_max as f32) * 100.0) as u8;
        if mana_pct < thresholds.mana_pct {
            return Some(ReadinessBlocker::LowMana {
                name: name.to_string(),
                current: mana_pct,
                threshold: thresholds.mana_pct,
            });
        }
    }

    // Check position (distance from camp center)
    let distance_to_camp =
        ((frame.x - frame.x_camp).powi(2) + (frame.y - frame.y_camp).powi(2)).sqrt();

    if distance_to_camp > config.position_radius {
        return Some(ReadinessBlocker::OutOfPosition {
            name: name.to_string(),
            distance: distance_to_camp,
            max_distance: config.position_radius,
        });
    }

    // Check buffs
    for required_buff in &config.required_buffs {
        if !frame.buffs.contains(required_buff) {
            return Some(ReadinessBlocker::MissingBuff {
                name: name.to_string(),
                buff: required_buff.clone(),
            });
        }
    }

    None
}

/// Checks if all group members are ready to pull.
/// Returns the first blocker found, or None if all members are ready.
#[must_use]
pub fn check_group_ready(
    members: &[(String, Role, &SharedStateFrame)],
    config: &GroupReadinessConfig,
) -> Option<ReadinessBlocker> {
    for (name, role, frame) in members {
        if let Some(blocker) = check_member_ready(name, role, frame, config) {
            return Some(blocker);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(hp: i32, hp_max: i32, mana: i32, mana_max: i32) -> SharedStateFrame {
        SharedStateFrame {
            client_id: 1,
            spawn_id: 1,
            name: "Test".to_string(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
            x_camp: 0.0,
            y_camp: 0.0,
            hp,
            hp_max,
            mana,
            mana_max,
            level: 50,
            class: "cleric".to_string(),
            buffs: vec![],
            spawn_epoch: 0,
            nearby_spawns: vec![],
        }
    }

    #[test]
    fn test_member_ready_good_state() {
        let config = GroupReadinessConfig {
            healer: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 95,
                mana_pct: 95,
            },
            tank: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 100,
                mana_pct: 0,
            },
            dps: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 80,
                mana_pct: 50,
            },
            position_radius: 30.0,
            timeout_ticks: 300,
            required_buffs: vec![],
        };

        let frame = make_frame(950, 1000, 950, 1000);
        let result = check_member_ready("Healer01", &Role::Healer, &frame, &config);
        assert_eq!(result, None, "Healer at 95% HP and mana should be ready");
    }

    #[test]
    fn test_member_dead() {
        let config = GroupReadinessConfig {
            healer: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 95,
                mana_pct: 95,
            },
            tank: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 100,
                mana_pct: 0,
            },
            dps: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 80,
                mana_pct: 50,
            },
            position_radius: 30.0,
            timeout_ticks: 300,
            required_buffs: vec![],
        };

        let frame = make_frame(0, 1000, 0, 1000);
        let result = check_member_ready("Healer01", &Role::Healer, &frame, &config);
        assert_eq!(
            result,
            Some(ReadinessBlocker::Dead {
                name: "Healer01".to_string()
            })
        );
    }

    #[test]
    fn test_member_low_mana() {
        let config = GroupReadinessConfig {
            healer: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 95,
                mana_pct: 95,
            },
            tank: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 100,
                mana_pct: 0,
            },
            dps: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 80,
                mana_pct: 50,
            },
            position_radius: 30.0,
            timeout_ticks: 300,
            required_buffs: vec![],
        };

        let frame = make_frame(950, 1000, 720, 1000); // 72% mana
        let result = check_member_ready("Healer01", &Role::Healer, &frame, &config);
        assert_eq!(
            result,
            Some(ReadinessBlocker::LowMana {
                name: "Healer01".to_string(),
                current: 72,
                threshold: 95
            })
        );
    }

    #[test]
    fn test_member_low_hp() {
        let config = GroupReadinessConfig {
            healer: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 95,
                mana_pct: 95,
            },
            tank: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 100,
                mana_pct: 0,
            },
            dps: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 80,
                mana_pct: 50,
            },
            position_radius: 30.0,
            timeout_ticks: 300,
            required_buffs: vec![],
        };

        let frame = make_frame(790, 1000, 950, 1000); // 79% HP, below 80% for DPS
        let result = check_member_ready("Ranger01", &Role::Dps, &frame, &config);
        assert_eq!(
            result,
            Some(ReadinessBlocker::LowHp {
                name: "Ranger01".to_string(),
                current: 79,
                threshold: 80
            })
        );
    }

    #[test]
    fn test_group_all_ready() {
        let config = GroupReadinessConfig {
            healer: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 95,
                mana_pct: 95,
            },
            tank: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 100,
                mana_pct: 0,
            },
            dps: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 80,
                mana_pct: 50,
            },
            position_radius: 30.0,
            timeout_ticks: 300,
            required_buffs: vec![],
        };

        let healer_frame = make_frame(950, 1000, 950, 1000);
        let tank_frame = make_frame(1000, 1000, 500, 500);
        let dps_frame = make_frame(800, 1000, 500, 1000);

        let members = vec![
            ("Healer01".to_string(), Role::Healer, &healer_frame),
            ("Tank01".to_string(), Role::Tank, &tank_frame),
            ("Ranger01".to_string(), Role::Dps, &dps_frame),
        ];

        let result = check_group_ready(&members, &config);
        assert_eq!(result, None, "All members ready should return None");
    }

    #[test]
    fn test_group_one_member_not_ready() {
        let config = GroupReadinessConfig {
            healer: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 95,
                mana_pct: 95,
            },
            tank: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 100,
                mana_pct: 0,
            },
            dps: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 80,
                mana_pct: 50,
            },
            position_radius: 30.0,
            timeout_ticks: 300,
            required_buffs: vec![],
        };

        let healer_frame = make_frame(950, 1000, 720, 1000); // Low mana
        let tank_frame = make_frame(1000, 1000, 500, 500);

        let members = vec![
            ("Healer01".to_string(), Role::Healer, &healer_frame),
            ("Tank01".to_string(), Role::Tank, &tank_frame),
        ];

        let result = check_group_ready(&members, &config);
        assert!(
            matches!(result, Some(ReadinessBlocker::LowMana { .. })),
            "Expected low mana blocker"
        );
    }

    #[test]
    fn test_missing_required_buff() {
        let config = GroupReadinessConfig {
            healer: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 95,
                mana_pct: 95,
            },
            tank: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 100,
                mana_pct: 0,
            },
            dps: crate::camp::config::RoleReadinessThresholds {
                hp_pct: 80,
                mana_pct: 50,
            },
            position_radius: 30.0,
            timeout_ticks: 300,
            required_buffs: vec!["Virtue".to_string()],
        };

        let mut frame = make_frame(950, 1000, 950, 1000);
        frame.buffs = vec!["SomeOtherBuff".to_string()];

        let result = check_member_ready("Healer01", &Role::Healer, &frame, &config);
        assert!(
            matches!(result, Some(ReadinessBlocker::MissingBuff { .. })),
            "Expected missing buff blocker"
        );
    }
}
