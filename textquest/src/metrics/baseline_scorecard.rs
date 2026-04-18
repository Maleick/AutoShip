//! Baseline Scorecard — measurement framework for optimization impact.
//!
//! Captures before/after metrics across four categories:
//! 1. Combat efficiency (DPS, resource consumption)
//! 2. Movement quality (stuck detection, route efficiency)
//! 3. Economy throughput (items/hour, plat/hour)
//! 4. Group coordination (assist timing, heal latency)
//!
//! Used for RL/optimization loop feedback during M9 learning phases.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Combat efficiency metrics — DPS proxy and resource management.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CombatMetrics {
    /// Average damage per second across all kills.
    pub dps: f64,
    /// Total mana consumed in the measurement window.
    pub mana_consumed: u32,
    /// Total endurance consumed in the measurement window.
    pub endurance_consumed: u32,
    /// Average pull-to-kill duration in seconds.
    pub avg_pull_to_kill_secs: f64,
    /// Total number of kills in the window.
    pub total_kills: u32,
}

impl CombatMetrics {
    /// Resource efficiency: DPS per mana point.
    pub fn dps_per_mana(&self) -> f64 {
        if self.mana_consumed == 0 {
            return 0.0;
        }
        self.dps / self.mana_consumed as f64
    }

    /// Resource efficiency: DPS per endurance point.
    pub fn dps_per_endurance(&self) -> f64 {
        if self.endurance_consumed == 0 {
            return 0.0;
        }
        self.dps / self.endurance_consumed as f64
    }
}

/// Movement quality metrics — navigation efficiency and failure detection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MovementMetrics {
    /// Percentage of time spent stuck (0.0 to 100.0).
    pub stuck_percentage: f64,
    /// Total distance traveled in game units.
    pub total_distance: f64,
    /// Number of stuck events detected.
    pub stuck_event_count: u32,
    /// Average route efficiency (actual distance / ideal distance, 0-1).
    pub route_efficiency: f64,
}

impl MovementMetrics {
    /// Quality score for movement (0-100).
    /// Higher efficiency and lower stuck percentage = higher score.
    pub fn quality_score(&self) -> f64 {
        let efficiency_component = self.route_efficiency * 50.0; // 0-50
        let stuck_component = (100.0 - self.stuck_percentage) * 0.5; // 0-50
        efficiency_component + stuck_component
    }
}

/// Economy throughput metrics — item and currency generation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EconomyMetrics {
    /// Items looted per hour.
    pub items_per_hour: f64,
    /// Platinum earned per hour.
    pub plat_per_hour: f64,
    /// Total items looted.
    pub total_items: u32,
    /// Total platinum earned.
    pub total_plat: u64,
    /// Duration of the measurement window in seconds.
    pub window_duration_secs: u64,
}

impl EconomyMetrics {
    /// Average item value (plat per item).
    pub fn avg_item_value(&self) -> f64 {
        if self.total_items == 0 {
            return 0.0;
        }
        self.total_plat as f64 / self.total_items as f64
    }
}

/// Group coordination metrics — team synchronization and response times.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupCoordinationMetrics {
    /// Average delay between assist request and execution (milliseconds).
    pub avg_assist_latency_ms: f64,
    /// Average delay between heal request and actual cast (milliseconds).
    pub avg_heal_response_latency_ms: f64,
    /// Number of failed assists (not executed within timeout).
    pub failed_assist_count: u32,
    /// Number of failed heals.
    pub failed_heal_count: u32,
    /// Synchronization score (0-100, measures coord timing precision).
    pub sync_score: f64,
}

impl GroupCoordinationMetrics {
    /// Reliability indicator: percentage of successful actions.
    pub fn success_rate(&self, total_actions: u32) -> f64 {
        if total_actions == 0 {
            return 0.0;
        }
        let failed = self.failed_assist_count + self.failed_heal_count;
        ((total_actions - failed) as f64 / total_actions as f64) * 100.0
    }
}

/// Complete baseline scorecard snapshot — all four metric categories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselineScorecard {
    /// Timestamp when this snapshot was captured (Unix epoch seconds).
    pub timestamp: i64,
    /// Combat efficiency metrics.
    pub combat: CombatMetrics,
    /// Movement quality metrics.
    pub movement: MovementMetrics,
    /// Economy throughput metrics.
    pub economy: EconomyMetrics,
    /// Group coordination metrics.
    pub coordination: GroupCoordinationMetrics,
}

impl BaselineScorecard {
    /// Create a new baseline scorecard snapshot.
    pub fn new(
        timestamp: i64,
        combat: CombatMetrics,
        movement: MovementMetrics,
        economy: EconomyMetrics,
        coordination: GroupCoordinationMetrics,
    ) -> Self {
        Self {
            timestamp,
            combat,
            movement,
            economy,
            coordination,
        }
    }

    /// Compute delta (difference) between two scorecards.
    /// Returns positive values where `after` > `before`.
    pub fn delta(&self, other: &BaselineScorecard) -> ScorecardDelta {
        ScorecardDelta {
            timestamp_delta_secs: other.timestamp - self.timestamp,
            combat_delta: CombatDelta {
                dps_delta: other.combat.dps - self.combat.dps,
                mana_consumed_delta: other.combat.mana_consumed as i32
                    - self.combat.mana_consumed as i32,
                endurance_consumed_delta: other.combat.endurance_consumed as i32
                    - self.combat.endurance_consumed as i32,
                avg_kill_time_delta_secs: other.combat.avg_pull_to_kill_secs
                    - self.combat.avg_pull_to_kill_secs,
                kills_delta: other.combat.total_kills as i32 - self.combat.total_kills as i32,
            },
            movement_delta: MovementDelta {
                stuck_percentage_delta: other.movement.stuck_percentage
                    - self.movement.stuck_percentage,
                distance_delta: other.movement.total_distance - self.movement.total_distance,
                route_efficiency_delta: other.movement.route_efficiency
                    - self.movement.route_efficiency,
            },
            economy_delta: EconomyDelta {
                items_per_hour_delta: other.economy.items_per_hour
                    - self.economy.items_per_hour,
                plat_per_hour_delta: other.economy.plat_per_hour - self.economy.plat_per_hour,
            },
            coordination_delta: CoordinationDelta {
                assist_latency_delta_ms: other.coordination.avg_assist_latency_ms
                    - self.coordination.avg_assist_latency_ms,
                heal_latency_delta_ms: other.coordination.avg_heal_response_latency_ms
                    - self.coordination.avg_heal_response_latency_ms,
                sync_score_delta: other.coordination.sync_score - self.coordination.sync_score,
            },
        }
    }

    /// Overall composite score (0-100) based on all metrics.
    /// Weights all categories equally for now.
    pub fn composite_score(&self) -> f64 {
        let combat_score = self.combat.dps.min(100.0); // Cap at 100
        let movement_score = self.movement.quality_score();
        let economy_score = self.economy.items_per_hour.min(100.0); // Cap at 100
        let coordination_score = self.coordination.sync_score;

        (combat_score + movement_score + economy_score + coordination_score) / 4.0
    }
}

/// Delta between two baseline scorecards — differences across all categories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScorecardDelta {
    /// Time elapsed between the two snapshots in seconds.
    pub timestamp_delta_secs: i64,
    /// Combat metric differences.
    pub combat_delta: CombatDelta,
    /// Movement metric differences.
    pub movement_delta: MovementDelta,
    /// Economy metric differences.
    pub economy_delta: EconomyDelta,
    /// Coordination metric differences.
    pub coordination_delta: CoordinationDelta,
}

/// Combat metric deltas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombatDelta {
    pub dps_delta: f64,
    pub mana_consumed_delta: i32,
    pub endurance_consumed_delta: i32,
    pub avg_kill_time_delta_secs: f64,
    pub kills_delta: i32,
}

/// Movement metric deltas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementDelta {
    pub stuck_percentage_delta: f64,
    pub distance_delta: f64,
    pub route_efficiency_delta: f64,
}

/// Economy metric deltas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EconomyDelta {
    pub items_per_hour_delta: f64,
    pub plat_per_hour_delta: f64,
}

/// Coordination metric deltas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinationDelta {
    pub assist_latency_delta_ms: f64,
    pub heal_latency_delta_ms: f64,
    pub sync_score_delta: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_metrics_dps_per_mana() {
        let metrics = CombatMetrics {
            dps: 100.0,
            mana_consumed: 200,
            endurance_consumed: 50,
            avg_pull_to_kill_secs: 10.0,
            total_kills: 5,
        };

        assert_eq!(metrics.dps_per_mana(), 0.5);
        assert_eq!(metrics.dps_per_endurance(), 2.0);
    }

    #[test]
    fn combat_metrics_zero_resource_consumption() {
        let metrics = CombatMetrics {
            dps: 50.0,
            mana_consumed: 0,
            endurance_consumed: 0,
            avg_pull_to_kill_secs: 5.0,
            total_kills: 2,
        };

        assert_eq!(metrics.dps_per_mana(), 0.0);
        assert_eq!(metrics.dps_per_endurance(), 0.0);
    }

    #[test]
    fn movement_metrics_quality_score() {
        let metrics = MovementMetrics {
            stuck_percentage: 10.0,
            total_distance: 500.0,
            stuck_event_count: 2,
            route_efficiency: 0.85,
        };

        let score = metrics.quality_score();
        // efficiency component: 0.85 * 50 = 42.5
        // stuck component: (100 - 10) * 0.5 = 45.0
        // total: 87.5
        assert!((score - 87.5).abs() < 0.01);
    }

    #[test]
    fn baseline_scorecard_delta() {
        let before = BaselineScorecard::new(
            1000,
            CombatMetrics {
                dps: 100.0,
                mana_consumed: 200,
                endurance_consumed: 100,
                avg_pull_to_kill_secs: 20.0,
                total_kills: 5,
            },
            MovementMetrics {
                stuck_percentage: 15.0,
                total_distance: 1000.0,
                stuck_event_count: 5,
                route_efficiency: 0.80,
            },
            EconomyMetrics {
                items_per_hour: 10.0,
                plat_per_hour: 500.0,
                total_items: 5,
                total_plat: 2500,
                window_duration_secs: 1800,
            },
            GroupCoordinationMetrics {
                avg_assist_latency_ms: 250.0,
                avg_heal_response_latency_ms: 150.0,
                failed_assist_count: 2,
                failed_heal_count: 1,
                sync_score: 75.0,
            },
        );

        let after = BaselineScorecard::new(
            2000,
            CombatMetrics {
                dps: 120.0,
                mana_consumed: 180,
                endurance_consumed: 120,
                avg_pull_to_kill_secs: 18.0,
                total_kills: 8,
            },
            MovementMetrics {
                stuck_percentage: 8.0,
                total_distance: 1200.0,
                stuck_event_count: 2,
                route_efficiency: 0.90,
            },
            EconomyMetrics {
                items_per_hour: 15.0,
                plat_per_hour: 700.0,
                total_items: 10,
                total_plat: 7000,
                window_duration_secs: 2400,
            },
            GroupCoordinationMetrics {
                avg_assist_latency_ms: 200.0,
                avg_heal_response_latency_ms: 120.0,
                failed_assist_count: 1,
                failed_heal_count: 0,
                sync_score: 85.0,
            },
        );

        let delta = before.delta(&after);

        assert_eq!(delta.timestamp_delta_secs, 1000);
        assert_eq!(delta.combat_delta.dps_delta, 20.0);
        assert_eq!(delta.combat_delta.mana_consumed_delta, -20);
        assert_eq!(delta.combat_delta.kills_delta, 3);
        assert!((delta.movement_delta.stuck_percentage_delta - (-7.0)).abs() < 0.01);
        assert_eq!(delta.movement_delta.route_efficiency_delta, 0.10);
        assert_eq!(delta.economy_delta.items_per_hour_delta, 5.0);
        assert_eq!(delta.economy_delta.plat_per_hour_delta, 200.0);
        assert_eq!(delta.coordination_delta.assist_latency_delta_ms, -50.0);
    }

    #[test]
    fn baseline_scorecard_composite_score() {
        let scorecard = BaselineScorecard::new(
            1000,
            CombatMetrics {
                dps: 80.0,
                mana_consumed: 100,
                endurance_consumed: 50,
                avg_pull_to_kill_secs: 15.0,
                total_kills: 4,
            },
            MovementMetrics {
                stuck_percentage: 5.0,
                total_distance: 800.0,
                stuck_event_count: 1,
                route_efficiency: 0.95,
            },
            EconomyMetrics {
                items_per_hour: 12.0,
                plat_per_hour: 600.0,
                total_items: 6,
                total_plat: 3600,
                window_duration_secs: 1800,
            },
            GroupCoordinationMetrics {
                avg_assist_latency_ms: 150.0,
                avg_heal_response_latency_ms: 100.0,
                failed_assist_count: 0,
                failed_heal_count: 0,
                sync_score: 90.0,
            },
        );

        let score = scorecard.composite_score();
        // combat: 80.0
        // movement: 0.95*50 + (100-5)*0.5 = 47.5 + 47.5 = 95.0
        // economy: 12.0 (capped at 100)
        // coordination: 90.0
        // avg: (80 + 95 + 12 + 90) / 4 = 69.25
        assert!((score - 69.25).abs() < 0.01);
    }

    #[test]
    fn economy_metrics_avg_item_value() {
        let metrics = EconomyMetrics {
            items_per_hour: 10.0,
            plat_per_hour: 500.0,
            total_items: 50,
            total_plat: 5000,
            window_duration_secs: 3600,
        };

        assert_eq!(metrics.avg_item_value(), 100.0);
    }

    #[test]
    fn group_coordination_success_rate() {
        let metrics = GroupCoordinationMetrics {
            avg_assist_latency_ms: 200.0,
            avg_heal_response_latency_ms: 150.0,
            failed_assist_count: 2,
            failed_heal_count: 1,
            sync_score: 80.0,
        };

        let success_rate = metrics.success_rate(20);
        // (20 - 3) / 20 * 100 = 85%
        assert!((success_rate - 85.0).abs() < 0.01);
    }
}
