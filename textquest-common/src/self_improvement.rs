//! Self-improvement loop — event recording, aggregation, and operator-in-the-loop tuning.
//!
//! Three-tier algorithm:
//! 1. Heuristics — rule-based pattern detection (tier 1, ships with MVP)
//! 2. Bayesian baseline — probabilistic behavior modeling (tier 2, optional)
//! 3. Multi-armed bandit — exploration/exploitation of config variants (tier 3, opt-in)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Raw session event recorded at runtime.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionEvent {
    /// Monotonic event ID.
    pub id: u64,
    /// Timestamp when event occurred.
    pub timestamp: DateTime<Utc>,
    /// Event type/category.
    pub event_type: EventType,
    /// Operator-readable description.
    pub description: String,
    /// Additional structured metadata (JSON-serializable).
    pub metadata: serde_json::Value,
}

/// Event types detected during a session.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Session started.
    SessionStart,
    /// Session ended normally or abnormally.
    SessionEnd,
    /// Character died.
    CharacterDeath,
    /// Combat engagement.
    CombatEngage,
    /// Combat ended.
    CombatEnd,
    /// Navigation waypoint reached.
    WaypointReached,
    /// Stuck detection triggered.
    StuckDetected,
    /// Configuration change.
    ConfigChange,
    /// GM alert triggered.
    GmAlert,
    /// Performance metric threshold exceeded.
    PerformanceAlert,
    /// Login succeeded.
    LoginSuccess,
    /// Login failed.
    LoginFailure,
    /// Custom application event.
    Custom(String),
}

/// Heuristic-based improvement suggestion (tier 1).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImprovementSuggestion {
    /// Unique suggestion ID.
    pub id: String,
    /// Title for operator display.
    pub title: String,
    /// Detailed explanation.
    pub description: String,
    /// Type of suggestion.
    pub suggestion_type: SuggestionType,
    /// Severity/priority (1=low, 5=critical).
    pub priority: u8,
    /// Suggested action or configuration change.
    pub recommendation: String,
    /// When suggestion was generated.
    pub created_at: DateTime<Utc>,
    /// Operator's acceptance status.
    pub status: SuggestionStatus,
    /// Config path to update if accepted (e.g., "config.combat.rotation").
    pub config_path: Option<String>,
    /// New value to apply if accepted.
    pub suggested_value: Option<serde_json::Value>,
}

/// Type of improvement suggestion.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionType {
    /// Performance optimization.
    Performance,
    /// Safety/detection-evasion improvement.
    Safety,
    /// Configuration tuning.
    ConfigTuning,
    /// Behavior adjustment.
    Behavior,
    /// Resource optimization.
    Resource,
    /// Combat rotation improvement.
    CombatRotation,
    /// Navigation optimization.
    Navigation,
    /// Other improvement.
    Other,
}

/// Operator's decision on a suggestion.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStatus {
    /// Pending operator review.
    Pending,
    /// Operator accepted the suggestion.
    Accepted,
    /// Operator rejected the suggestion.
    Rejected,
    /// Suggestion applied to config.
    Applied,
    /// Suggestion undone/reverted.
    Undone,
}

/// Aggregated metrics for a session.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SessionMetrics {
    /// Total session duration in seconds.
    pub duration_secs: u64,
    /// Character death count.
    pub deaths: u32,
    /// Combat engagements.
    pub combats: u32,
    /// Waypoints reached.
    pub waypoints_completed: u32,
    /// Stuck detections.
    pub stuck_count: u32,
    /// GM alerts triggered.
    pub gm_alerts: u32,
    /// Login attempts.
    pub login_attempts: u32,
    /// Custom metrics from events.
    pub custom_metrics: HashMap<String, f64>,
}

/// Heuristic rules engine for tier 1 suggestions.
#[derive(Clone, Debug)]
pub struct HeuristicEngine {
    /// Rules that have been evaluated.
    pub rules: Vec<HeuristicRule>,
}

/// Single heuristic rule to detect patterns.
#[derive(Clone, Debug)]
pub struct HeuristicRule {
    /// Rule name/ID.
    pub id: String,
    /// Rule predicate (e.g., "deaths > 5 in 1 hour").
    pub name: String,
    /// Suggestion to emit if rule triggers.
    pub suggestion: ImprovementSuggestion,
}

impl HeuristicEngine {
    /// Create a new heuristic engine with default rules.
    pub fn new() -> Self {
        HeuristicEngine {
            rules: vec![
                // Rule: High death rate suggests combat rotation issue
                HeuristicRule {
                    id: "high_death_rate".to_string(),
                    name: "Multiple character deaths detected".to_string(),
                    suggestion: ImprovementSuggestion {
                        id: "suggest_combat_rotation".to_string(),
                        title: "Review combat rotation".to_string(),
                        description: "Multiple deaths detected. Consider reviewing combat rotation and assist target settings.".to_string(),
                        suggestion_type: SuggestionType::CombatRotation,
                        priority: 4,
                        recommendation: "Check combat rotation priority, spell selection, and healing chain configuration.".to_string(),
                        created_at: Utc::now(),
                        status: SuggestionStatus::Pending,
                        config_path: Some("combat.rotation".to_string()),
                        suggested_value: None,
                    },
                },
                // Rule: Frequent stuck detections suggest nav issue
                HeuristicRule {
                    id: "stuck_detection_freq".to_string(),
                    name: "Frequent stuck detections".to_string(),
                    suggestion: ImprovementSuggestion {
                        id: "suggest_nav_tuning".to_string(),
                        title: "Optimize navigation settings".to_string(),
                        description: "The navigation system triggered stuck detection frequently. Consider adjusting waypoint spacing or obstacle detection sensitivity.".to_string(),
                        suggestion_type: SuggestionType::Navigation,
                        priority: 3,
                        recommendation: "Reduce waypoint distance or increase stuck detection threshold.".to_string(),
                        created_at: Utc::now(),
                        status: SuggestionStatus::Pending,
                        config_path: Some("navigation.stuck_threshold".to_string()),
                        suggested_value: None,
                    },
                },
                // Rule: GM alerts suggest stealth review
                HeuristicRule {
                    id: "gm_alerts_detected".to_string(),
                    name: "GM alert triggered".to_string(),
                    suggestion: ImprovementSuggestion {
                        id: "suggest_stealth_review".to_string(),
                        title: "Review stealth and detection evasion".to_string(),
                        description: "GM detection alert was triggered. Review character behavior and account safety settings.".to_string(),
                        suggestion_type: SuggestionType::Safety,
                        priority: 5,
                        recommendation: "Review behavior patterns, reduce automation visibility, and consider account safety settings.".to_string(),
                        created_at: Utc::now(),
                        status: SuggestionStatus::Pending,
                        config_path: None,
                        suggested_value: None,
                    },
                },
            ],
        }
    }
}

impl Default for HeuristicEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluate events and generate suggestions based on heuristic rules.
pub fn analyze_session_for_suggestions(
    metrics: &SessionMetrics,
    _events: &[SessionEvent],
) -> Vec<ImprovementSuggestion> {
    let mut suggestions = Vec::new();

    // Heuristic 1: High death rate (>5 deaths)
    if metrics.deaths > 5 {
        suggestions.push(ImprovementSuggestion {
            id: format!("suggest_combat_rotation_{}", chrono::Utc::now().timestamp()),
            title: "Review combat rotation — high death rate".to_string(),
            description: format!(
                "Session had {} deaths. Consider reviewing combat rotation, healing chain, or assist target settings.",
                metrics.deaths
            ),
            suggestion_type: SuggestionType::CombatRotation,
            priority: 4,
            recommendation: "Check melee rotation priority, spell selection, and healing order.".to_string(),
            created_at: Utc::now(),
            status: SuggestionStatus::Pending,
            config_path: Some("combat.rotation".to_string()),
            suggested_value: None,
        });
    }

    // Heuristic 2: Frequent stuck detection (>10 stucks)
    if metrics.stuck_count > 10 {
        suggestions.push(ImprovementSuggestion {
            id: format!("suggest_nav_tuning_{}", chrono::Utc::now().timestamp()),
            title: "Optimize navigation — frequent stuck detection".to_string(),
            description: format!(
                "Navigation triggered stuck detection {} times. Consider waypoint spacing or pathfinding adjustments.",
                metrics.stuck_count
            ),
            suggestion_type: SuggestionType::Navigation,
            priority: 3,
            recommendation: "Reduce waypoint distance or increase stuck-detection threshold.".to_string(),
            created_at: Utc::now(),
            status: SuggestionStatus::Pending,
            config_path: Some("navigation.stuck_threshold".to_string()),
            suggested_value: Some(serde_json::json!(8.0)), // Example: increase from 6.0 to 8.0
        });
    }

    // Heuristic 3: GM alert detected (>0)
    if metrics.gm_alerts > 0 {
        suggestions.push(ImprovementSuggestion {
            id: format!("suggest_stealth_review_{}", chrono::Utc::now().timestamp()),
            title: "Review safety — GM detection triggered".to_string(),
            description:
                "GM detection was triggered. Review character behavior and account safety."
                    .to_string(),
            suggestion_type: SuggestionType::Safety,
            priority: 5,
            recommendation: "Reduce automation visibility and review behavior patterns."
                .to_string(),
            created_at: Utc::now(),
            status: SuggestionStatus::Pending,
            config_path: None,
            suggested_value: None,
        });
    }

    // Heuristic 4: Long session with low combat (performance check)
    if metrics.duration_secs > 3600 && metrics.combats < 10 {
        suggestions.push(ImprovementSuggestion {
            id: format!("suggest_camp_tuning_{}", chrono::Utc::now().timestamp()),
            title: "Review camp settings — low combat activity".to_string(),
            description: "Long session with low combat. Consider camp location or pull radius."
                .to_string(),
            suggestion_type: SuggestionType::ConfigTuning,
            priority: 2,
            recommendation: "Check camp pull radius or relocate camp to higher spawn area."
                .to_string(),
            created_at: Utc::now(),
            status: SuggestionStatus::Pending,
            config_path: Some("camp.pull_radius".to_string()),
            suggested_value: None,
        });
    }

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_engine_creation() {
        let engine = HeuristicEngine::new();
        assert!(!engine.rules.is_empty());
    }

    #[test]
    fn test_high_death_rate_suggestion() {
        let metrics = SessionMetrics {
            deaths: 10,
            ..Default::default()
        };
        let suggestions = analyze_session_for_suggestions(&metrics, &[]);
        assert!(
            suggestions
                .iter()
                .any(|s| s.suggestion_type == SuggestionType::CombatRotation)
        );
    }

    #[test]
    fn test_stuck_detection_suggestion() {
        let metrics = SessionMetrics {
            stuck_count: 15,
            ..Default::default()
        };
        let suggestions = analyze_session_for_suggestions(&metrics, &[]);
        assert!(
            suggestions
                .iter()
                .any(|s| s.suggestion_type == SuggestionType::Navigation)
        );
    }

    #[test]
    fn test_gm_alert_suggestion() {
        let metrics = SessionMetrics {
            gm_alerts: 1,
            ..Default::default()
        };
        let suggestions = analyze_session_for_suggestions(&metrics, &[]);
        assert!(
            suggestions
                .iter()
                .any(|s| s.suggestion_type == SuggestionType::Safety)
        );
    }
}
