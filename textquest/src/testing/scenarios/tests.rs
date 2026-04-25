use super::*;
use crate::testing::scenario::{MetricValue, TestScenario};
use std::time::Duration;

#[tokio::test]
async fn camp_loop_initializes_with_config_and_metrics() {
    let config = CampLoopConfig::new("orc_highway", 4);
    let mut scenario = CampLoopScenario::new(config.clone());

    assert_eq!(scenario.name(), "camp_loop");
    assert_eq!(scenario.config(), &config);

    let result = scenario.run(Duration::from_millis(250)).await;

    assert!(result.success);
    assert!(result.errors.is_empty());
    assert_eq!(result.duration, Duration::from_millis(250));
    assert_eq!(result.metrics["camp.target_loops"], MetricValue::Counter(4));
    assert_eq!(
        result.metrics["camp.completed_loops"],
        MetricValue::Counter(4)
    );
    assert_eq!(
        result.metrics["camp.early_terminations"],
        MetricValue::Counter(0)
    );
    assert_eq!(result.metrics["camp.duration_ms"], MetricValue::Counter(250));
}

#[tokio::test]
async fn camp_loop_reports_early_termination() {
    let config = CampLoopConfig::new("orc_highway", 5).with_early_termination(2);
    let mut scenario = CampLoopScenario::new(config);

    let result = scenario.run(Duration::from_secs(1)).await;

    assert!(!result.success);
    assert_eq!(
        result.metrics["camp.completed_loops"],
        MetricValue::Counter(2)
    );
    assert_eq!(
        result.metrics["camp.early_terminations"],
        MetricValue::Counter(1)
    );
    assert_eq!(
        result.errors,
        vec!["camp 'orc_highway' terminated after 2 loops"]
    );
}

#[test]
fn navigation_parses_and_validates_routes() {
    let scenario = NavigationScenario::from_route("qeynos -> blackburrow -> halas").unwrap();

    assert_eq!(
        scenario.route(),
        &[
            "qeynos".to_string(),
            "blackburrow".to_string(),
            "halas".to_string()
        ]
    );
    assert_eq!(
        NavigationScenario::from_route("qeynos").unwrap_err(),
        "route must contain at least two waypoints"
    );
    assert_eq!(
        NavigationScenario::from_route("qeynos ->  -> halas").unwrap_err(),
        "route contains an empty waypoint"
    );
}

#[tokio::test]
async fn navigation_metrics_count_completed_waypoints() {
    let mut scenario = NavigationScenario::from_route("a -> b -> c").unwrap();

    let result = scenario.run(Duration::from_millis(90)).await;

    assert!(result.success);
    assert_eq!(
        result.metrics["navigation.waypoints_total"],
        MetricValue::Counter(3)
    );
    assert_eq!(
        result.metrics["navigation.waypoints_completed"],
        MetricValue::Counter(3)
    );
    assert_eq!(
        result.metrics["navigation.duration_ms"],
        MetricValue::Counter(90)
    );
}

#[tokio::test]
async fn navigation_failure_stops_at_failed_waypoint() {
    let mut scenario = NavigationScenario::from_route("a -> b -> c")
        .unwrap()
        .with_failure_at(1);

    let result = scenario.run(Duration::from_secs(1)).await;

    assert!(!result.success);
    assert_eq!(
        result.metrics["navigation.waypoints_completed"],
        MetricValue::Counter(1)
    );
    assert_eq!(result.errors, vec!["failed to reach waypoint 'b'"]);
}

#[tokio::test]
async fn combat_rotation_initializes_each_class() {
    for (class, abilities) in [
        (CombatClass::Warrior, &["taunt", "kick", "bash"][..]),
        (CombatClass::Cleric, &["smite", "heal", "stun"][..]),
        (CombatClass::Wizard, &["nuke", "snare", "evacuate"][..]),
    ] {
        let scenario = CombatRotationScenario::for_class(class);

        assert_eq!(scenario.class(), class);
        assert_eq!(scenario.abilities(), abilities);
    }
}

#[tokio::test]
async fn combat_rotation_counts_rotations_and_casts() {
    let mut scenario = CombatRotationScenario::for_class(CombatClass::Wizard).with_rotations(3);

    let result = scenario.run(Duration::from_millis(500)).await;

    assert!(result.success);
    assert_eq!(
        result.metrics["combat.rotations"],
        MetricValue::Counter(3)
    );
    assert_eq!(
        result.metrics["combat.ability_casts"],
        MetricValue::Counter(9)
    );
    assert_eq!(
        result.metrics["combat.abilities_per_rotation"],
        MetricValue::Counter(3)
    );
}

#[tokio::test]
async fn countdown_scenario_uses_timing_budget() {
    let mut scenario = MockScenarios::countdown(5, Duration::from_millis(10));

    let result = scenario.run(Duration::from_millis(30)).await;

    assert!(result.success);
    assert_eq!(result.duration, Duration::from_millis(30));
    assert_eq!(
        result.metrics["countdown.ticks_requested"],
        MetricValue::Counter(5)
    );
    assert_eq!(
        result.metrics["countdown.ticks_completed"],
        MetricValue::Counter(3)
    );
}

#[tokio::test]
async fn metric_test_scenario_emits_expected_values() {
    let mut scenario = MockScenarios::metric_test();

    let result = scenario.run(Duration::from_millis(1)).await;

    assert!(result.success);
    assert_eq!(result.metrics["metric.counter"], MetricValue::Counter(7));
    assert_eq!(result.metrics["metric.gauge"], MetricValue::Gauge(3.5));
    assert_eq!(
        result.metrics["metric.histogram"],
        MetricValue::Histogram(vec![1.0, 2.0, 3.0])
    );
}

#[tokio::test]
async fn fast_fail_scenario_returns_error_immediately() {
    let mut scenario = MockScenarios::fast_fail("boom");

    let result = scenario.run(Duration::from_secs(10)).await;

    assert!(!result.success);
    assert_eq!(result.duration, Duration::ZERO);
    assert_eq!(result.errors, vec!["boom"]);
    assert_eq!(
        result.metrics["fast_fail.errors"],
        MetricValue::Counter(1)
    );
}
