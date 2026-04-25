//! Navigation test scenario for waypoint routing and camp-transition smoke tests.

use std::{collections::HashMap, time::Duration};

#[cfg(windows)]
use crate::nav::router::{GroupRouter, TravelPlan, TravelStep};
use crate::testing::scenario::{BoxScenarioFuture, MetricValue, ScenarioResult, TestScenario};
use textquest_common::{nav::Waypoint, types::ClientId};

const DEFAULT_CLIENT_ID: ClientId = 1;
const DEFAULT_CLASS_ID: u8 = 1;
const DEFAULT_ZONE: &str = "scenario";
const DEFAULT_WAYPOINT_TIMEOUT: Duration = Duration::from_secs(30);

/// Result of one navigator FSM waypoint attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct NavigationStepResult {
    /// Whether the waypoint was reached.
    pub reached: bool,
    /// Simulated or observed time spent on the waypoint.
    pub elapsed: Duration,
    /// Stuck detector triggers observed during the attempt.
    pub stuck_detections: u64,
    /// Whether this waypoint exercised a camp transition.
    pub camp_transition: bool,
    /// Optional failure detail.
    pub error: Option<String>,
}

impl NavigationStepResult {
    /// Successful waypoint traversal.
    #[must_use]
    pub fn reached(elapsed: Duration) -> Self {
        Self {
            reached: true,
            elapsed,
            stuck_detections: 0,
            camp_transition: false,
            error: None,
        }
    }

    /// Failed waypoint traversal.
    #[must_use]
    pub fn failed(elapsed: Duration, error: impl Into<String>) -> Self {
        Self {
            reached: false,
            elapsed,
            stuck_detections: 0,
            camp_transition: false,
            error: Some(error.into()),
        }
    }

    /// Add stuck detector triggers to this result.
    #[must_use]
    pub fn with_stuck_detections(mut self, count: u64) -> Self {
        self.stuck_detections = count;
        self
    }

    /// Mark this result as crossing a camp transition.
    #[must_use]
    pub fn with_camp_transition(mut self) -> Self {
        self.camp_transition = true;
        self
    }
}

/// Router-produced movement plan handed to the navigator FSM.
pub struct NavigationPlan {
    #[cfg(windows)]
    inner: TravelPlan,
    #[cfg(not(windows))]
    waypoints: Vec<Waypoint>,
}

impl NavigationPlan {
    fn walk_to(client_id: ClientId, waypoint: Waypoint) -> Self {
        #[cfg(windows)]
        {
            Self {
                inner: TravelPlan::new(
                    client_id,
                    vec![TravelStep::WalkTo {
                        waypoints: vec![waypoint],
                    }],
                ),
            }
        }

        #[cfg(not(windows))]
        {
            let _ = client_id;
            Self {
                waypoints: vec![waypoint],
            }
        }
    }

    fn has_current(&self) -> bool {
        #[cfg(windows)]
        {
            self.inner.current().is_some()
        }

        #[cfg(not(windows))]
        {
            !self.waypoints.is_empty()
        }
    }

    /// Access the underlying router plan on Windows navigation builds.
    #[cfg(windows)]
    #[must_use]
    pub fn travel_plan(&self) -> &TravelPlan {
        &self.inner
    }
}

/// Minimal Navigator FSM surface used by [`NavigationScenario`].
pub trait NavigatorFsm: Send {
    /// Navigate one waypoint using the router-generated travel plan.
    fn navigate_to(&mut self, waypoint: Waypoint, plan: &NavigationPlan) -> NavigationStepResult;

    /// Release any movement/camp state held by the navigator.
    fn cleanup(&mut self);
}

#[derive(Debug, Default)]
struct SimulatedNavigatorFsm {
    current_position: Option<Waypoint>,
}

impl NavigatorFsm for SimulatedNavigatorFsm {
    fn navigate_to(&mut self, waypoint: Waypoint, plan: &NavigationPlan) -> NavigationStepResult {
        if !plan.has_current() {
            return NavigationStepResult::failed(Duration::ZERO, "router produced an empty plan");
        }

        let distance = self
            .current_position
            .map(|current| current.distance_3d(&waypoint))
            .unwrap_or(0.0);
        self.current_position = Some(waypoint);

        let millis = (distance.ceil() as u64).max(1);
        NavigationStepResult::reached(Duration::from_millis(millis))
    }

    fn cleanup(&mut self) {
        self.current_position = None;
    }
}

#[derive(Debug, Default)]
struct NavigationMetrics {
    waypoints_visited: u64,
    navigation_failures: u64,
    stuck_detections: u64,
}

/// Scenario that validates waypoint routing through `GroupRouter` and a navigator FSM.
pub struct NavigationScenario {
    waypoints: Vec<Waypoint>,
    max_duration: Duration,
    waypoint_timeout: Duration,
    #[cfg(windows)]
    router: GroupRouter,
    navigator: Box<dyn NavigatorFsm>,
    client_ids: Vec<ClientId>,
    #[cfg_attr(not(windows), allow(dead_code))]
    class_map: HashMap<ClientId, u8>,
    #[cfg_attr(not(windows), allow(dead_code))]
    current_zone: String,
}

impl NavigationScenario {
    /// Create a scenario for the given waypoint list and run budget.
    #[must_use]
    pub fn new(waypoints: Vec<Waypoint>, max_duration: Duration) -> Self {
        let client_ids = vec![DEFAULT_CLIENT_ID];
        let class_map = HashMap::from([(DEFAULT_CLIENT_ID, DEFAULT_CLASS_ID)]);

        Self {
            waypoints,
            max_duration,
            waypoint_timeout: DEFAULT_WAYPOINT_TIMEOUT,
            #[cfg(windows)]
            router: GroupRouter::new(),
            navigator: Box::<SimulatedNavigatorFsm>::default(),
            client_ids,
            class_map,
            current_zone: DEFAULT_ZONE.to_string(),
        }
    }

    /// Override the per-waypoint timeout.
    #[must_use]
    pub fn with_waypoint_timeout(mut self, waypoint_timeout: Duration) -> Self {
        self.waypoint_timeout = waypoint_timeout;
        self
    }

    /// Override group routing inputs.
    #[must_use]
    pub fn with_group(
        mut self,
        client_ids: Vec<ClientId>,
        class_map: HashMap<ClientId, u8>,
        current_zone: impl Into<String>,
    ) -> Self {
        self.client_ids = client_ids;
        self.class_map = class_map;
        self.current_zone = current_zone.into();
        self
    }

    /// Override the router implementation state.
    #[cfg(windows)]
    #[must_use]
    pub fn with_router(mut self, router: GroupRouter) -> Self {
        self.router = router;
        self
    }

    /// Override the navigator FSM. Intended for deterministic tests and harness adapters.
    #[must_use]
    pub fn with_navigator(mut self, navigator: impl NavigatorFsm + 'static) -> Self {
        self.navigator = Box::new(navigator);
        self
    }

    fn run_once(&mut self, duration: Duration) -> ScenarioResult {
        let budget = duration.min(self.max_duration);
        let mut elapsed = Duration::ZERO;
        let mut metrics = NavigationMetrics::default();
        let mut errors = Vec::new();
        let total_waypoints = self.waypoints.len() as u64;

        if let Some(error) = self.group_route_error() {
            metrics.navigation_failures += 1;
            errors.push(error);
        }

        if errors.is_empty() {
            for (index, waypoint) in self.waypoints.iter().copied().enumerate() {
                if elapsed >= budget {
                    metrics.navigation_failures += 1;
                    errors.push(format!("duration budget expired before waypoint {index}"));
                    break;
                }

                let plan = NavigationPlan::walk_to(self.primary_client_id(), waypoint);
                let outcome = self.navigator.navigate_to(waypoint, &plan);
                metrics.stuck_detections += outcome.stuck_detections;

                if outcome.elapsed > self.waypoint_timeout {
                    metrics.navigation_failures += 1;
                    elapsed = elapsed.saturating_add(self.waypoint_timeout);
                    errors.push(format!(
                        "waypoint {index} timed out after {:.3}s",
                        self.waypoint_timeout.as_secs_f64()
                    ));
                    break;
                }

                if elapsed.saturating_add(outcome.elapsed) > budget {
                    metrics.navigation_failures += 1;
                    elapsed = budget;
                    errors.push(format!("duration budget expired during waypoint {index}"));
                    break;
                }

                elapsed += outcome.elapsed;

                if outcome.reached {
                    metrics.waypoints_visited += 1;
                } else {
                    metrics.navigation_failures += 1;
                    errors.push(
                        outcome.error.unwrap_or_else(|| {
                            format!("navigator failed to reach waypoint {index}")
                        }),
                    );
                    break;
                }
            }
        }

        self.navigator.cleanup();
        self.result(elapsed, metrics, total_waypoints, errors)
    }

    fn primary_client_id(&self) -> ClientId {
        self.client_ids
            .first()
            .copied()
            .unwrap_or(DEFAULT_CLIENT_ID)
    }

    fn group_route_error(&self) -> Option<String> {
        #[cfg(windows)]
        {
            let router_plans = self.router.plan_travel(
                &self.client_ids,
                &self.class_map,
                &self.current_zone,
                &self.current_zone,
            );
            if !self.client_ids.is_empty() && router_plans.len() != self.client_ids.len() {
                return Some(format!(
                    "router returned {} plans for {} clients",
                    router_plans.len(),
                    self.client_ids.len()
                ));
            }
        }

        None
    }

    fn result(
        &self,
        duration: Duration,
        metrics: NavigationMetrics,
        total_waypoints: u64,
        errors: Vec<String>,
    ) -> ScenarioResult {
        let success_rate = if total_waypoints == 0 {
            1.0
        } else {
            metrics.waypoints_visited as f64 / total_waypoints as f64
        };
        let avg_time_per_waypoint = if total_waypoints == 0 {
            0.0
        } else {
            duration.as_secs_f64() / total_waypoints as f64
        };

        let result = if errors.is_empty() && metrics.waypoints_visited == total_waypoints {
            ScenarioResult::success(duration)
        } else {
            ScenarioResult::failure(duration, errors)
        };

        result
            .with_metric(
                "waypoints_visited",
                MetricValue::Counter(metrics.waypoints_visited),
            )
            .with_metric(
                "navigation_failures",
                MetricValue::Counter(metrics.navigation_failures),
            )
            .with_metric(
                "stuck_detections",
                MetricValue::Counter(metrics.stuck_detections),
            )
            .with_metric(
                "avg_time_per_waypoint",
                MetricValue::Gauge(avg_time_per_waypoint),
            )
            .with_metric("success_rate", MetricValue::Gauge(success_rate))
    }
}

impl TestScenario for NavigationScenario {
    fn name(&self) -> &str {
        "navigation"
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async move { self.run_once(duration) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    struct MockNavigator {
        outcomes: VecDeque<NavigationStepResult>,
        cleanup_called: Arc<AtomicBool>,
    }

    impl MockNavigator {
        fn new(outcomes: Vec<NavigationStepResult>) -> (Self, Arc<AtomicBool>) {
            let cleanup_called = Arc::new(AtomicBool::new(false));
            (
                Self {
                    outcomes: outcomes.into(),
                    cleanup_called: Arc::clone(&cleanup_called),
                },
                cleanup_called,
            )
        }
    }

    impl NavigatorFsm for MockNavigator {
        fn navigate_to(
            &mut self,
            _waypoint: Waypoint,
            _plan: &NavigationPlan,
        ) -> NavigationStepResult {
            self.outcomes
                .pop_front()
                .unwrap_or_else(|| NavigationStepResult::failed(Duration::ZERO, "missing outcome"))
        }

        fn cleanup(&mut self) {
            self.cleanup_called.store(true, Ordering::SeqCst);
        }
    }

    fn waypoint(index: f32) -> Waypoint {
        Waypoint::new(index, index * 2.0, 0.0)
    }

    fn counter(result: &ScenarioResult, name: &str) -> u64 {
        match result.metrics.get(name) {
            Some(MetricValue::Counter(value)) => *value,
            other => panic!("expected counter metric {name}, got {other:?}"),
        }
    }

    fn gauge(result: &ScenarioResult, name: &str) -> f64 {
        match result.metrics.get(name) {
            Some(MetricValue::Gauge(value)) => *value,
            other => panic!("expected gauge metric {name}, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn linear_route_navigation_visits_all_waypoints() {
        let (navigator, cleanup_called) = MockNavigator::new(vec![
            NavigationStepResult::reached(Duration::from_millis(10)),
            NavigationStepResult::reached(Duration::from_millis(10)),
            NavigationStepResult::reached(Duration::from_millis(10)),
        ]);
        let mut scenario = NavigationScenario::new(
            vec![waypoint(1.0), waypoint(2.0), waypoint(3.0)],
            Duration::from_secs(5),
        )
        .with_navigator(navigator);

        let result = scenario.run(Duration::from_secs(5)).await;

        assert!(result.success);
        assert!(cleanup_called.load(Ordering::SeqCst));
        assert_eq!(counter(&result, "waypoints_visited"), 3);
        assert_eq!(counter(&result, "navigation_failures"), 0);
        assert_eq!(gauge(&result, "success_rate"), 1.0);
        assert!((gauge(&result, "avg_time_per_waypoint") - 0.010).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn circular_route_navigation_accepts_repeated_endpoint() {
        let mut scenario = NavigationScenario::new(
            vec![waypoint(1.0), waypoint(2.0), waypoint(3.0), waypoint(1.0)],
            Duration::from_secs(5),
        );

        let result = scenario.run(Duration::from_secs(5)).await;

        assert!(result.success);
        assert_eq!(counter(&result, "waypoints_visited"), 4);
        assert_eq!(gauge(&result, "success_rate"), 1.0);
    }

    #[tokio::test]
    async fn stuck_detection_recovery_counts_trigger_and_continues() {
        let (navigator, _cleanup_called) = MockNavigator::new(vec![
            NavigationStepResult::reached(Duration::from_millis(10)).with_stuck_detections(1),
            NavigationStepResult::reached(Duration::from_millis(10)),
        ]);
        let mut scenario =
            NavigationScenario::new(vec![waypoint(1.0), waypoint(2.0)], Duration::from_secs(5))
                .with_navigator(navigator);

        let result = scenario.run(Duration::from_secs(5)).await;

        assert!(result.success);
        assert_eq!(counter(&result, "waypoints_visited"), 2);
        assert_eq!(counter(&result, "stuck_detections"), 1);
    }

    #[tokio::test]
    async fn waypoint_timeout_reports_failure_and_cleans_up() {
        let (navigator, cleanup_called) =
            MockNavigator::new(vec![NavigationStepResult::reached(Duration::from_secs(2))]);
        let mut scenario = NavigationScenario::new(vec![waypoint(1.0)], Duration::from_secs(5))
            .with_waypoint_timeout(Duration::from_millis(100))
            .with_navigator(navigator);

        let result = scenario.run(Duration::from_secs(5)).await;

        assert!(!result.success);
        assert!(cleanup_called.load(Ordering::SeqCst));
        assert_eq!(counter(&result, "waypoints_visited"), 0);
        assert_eq!(counter(&result, "navigation_failures"), 1);
        assert!(result.errors[0].contains("timed out"));
    }

    #[tokio::test]
    async fn mock_route_failure_records_navigation_failure() {
        let (navigator, cleanup_called) = MockNavigator::new(vec![NavigationStepResult::failed(
            Duration::from_millis(5),
            "mock route blocked",
        )]);
        let mut scenario = NavigationScenario::new(vec![waypoint(1.0)], Duration::from_secs(5))
            .with_navigator(navigator);

        let result = scenario.run(Duration::from_secs(5)).await;

        assert!(!result.success);
        assert!(cleanup_called.load(Ordering::SeqCst));
        assert_eq!(counter(&result, "waypoints_visited"), 0);
        assert_eq!(counter(&result, "navigation_failures"), 1);
        assert_eq!(gauge(&result, "success_rate"), 0.0);
        assert_eq!(result.errors, vec!["mock route blocked".to_string()]);
    }
}
