//! Deterministic `TestScenario` implementations used by the test harness.

use std::time::Duration;

use super::scenario::{BoxScenarioFuture, MetricValue, ScenarioResult, TestScenario};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampLoopConfig {
    pub camp_name: String,
    pub target_loops: u64,
    pub terminate_after: Option<u64>,
}

impl CampLoopConfig {
    pub fn new(camp_name: impl Into<String>, target_loops: u64) -> Self {
        Self {
            camp_name: camp_name.into(),
            target_loops,
            terminate_after: None,
        }
    }

    pub fn with_early_termination(mut self, terminate_after: u64) -> Self {
        self.terminate_after = Some(terminate_after);
        self
    }
}

#[derive(Debug, Clone)]
pub struct CampLoopScenario {
    config: CampLoopConfig,
}

impl CampLoopScenario {
    pub fn new(config: CampLoopConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &CampLoopConfig {
        &self.config
    }
}

impl TestScenario for CampLoopScenario {
    fn name(&self) -> &str {
        "camp_loop"
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async move {
            let terminated = self
                .config
                .terminate_after
                .is_some_and(|limit| limit < self.config.target_loops);
            let completed = self
                .config
                .terminate_after
                .map_or(self.config.target_loops, |limit| {
                    limit.min(self.config.target_loops)
                });
            let base = if terminated {
                ScenarioResult::failure(
                    duration,
                    vec![format!(
                        "camp '{}' terminated after {completed} loops",
                        self.config.camp_name
                    )],
                )
            } else {
                ScenarioResult::success(duration)
            };

            base.with_metric(
                "camp.target_loops",
                MetricValue::Counter(self.config.target_loops),
            )
            .with_metric("camp.completed_loops", MetricValue::Counter(completed))
            .with_metric(
                "camp.early_terminations",
                MetricValue::Counter(u64::from(terminated)),
            )
            .with_metric("camp.duration_ms", MetricValue::Counter(duration_millis(duration)))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationScenario {
    route: Vec<String>,
    fail_at: Option<usize>,
}

impl NavigationScenario {
    pub fn from_route(route: &str) -> Result<Self, String> {
        let waypoints = route
            .split("->")
            .map(str::trim)
            .map(str::to_string)
            .collect::<Vec<_>>();

        if waypoints.len() < 2 {
            return Err("route must contain at least two waypoints".to_string());
        }
        if waypoints.iter().any(String::is_empty) {
            return Err("route contains an empty waypoint".to_string());
        }

        Ok(Self {
            route: waypoints,
            fail_at: None,
        })
    }

    pub fn with_failure_at(mut self, waypoint_index: usize) -> Self {
        self.fail_at = Some(waypoint_index);
        self
    }

    pub fn route(&self) -> &[String] {
        &self.route
    }
}

impl TestScenario for NavigationScenario {
    fn name(&self) -> &str {
        "navigation"
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async move {
            let failed_index = self.fail_at.filter(|index| *index < self.route.len());
            let completed = failed_index.unwrap_or(self.route.len());
            let base = if let Some(index) = failed_index {
                ScenarioResult::failure(
                    duration,
                    vec![format!("failed to reach waypoint '{}'", self.route[index])],
                )
            } else {
                ScenarioResult::success(duration)
            };

            base.with_metric(
                "navigation.waypoints_total",
                MetricValue::Counter(self.route.len() as u64),
            )
            .with_metric(
                "navigation.waypoints_completed",
                MetricValue::Counter(completed as u64),
            )
            .with_metric(
                "navigation.duration_ms",
                MetricValue::Counter(duration_millis(duration)),
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatClass {
    Warrior,
    Cleric,
    Wizard,
}

impl CombatClass {
    pub fn abilities(self) -> &'static [&'static str] {
        match self {
            Self::Warrior => &["taunt", "kick", "bash"],
            Self::Cleric => &["smite", "heal", "stun"],
            Self::Wizard => &["nuke", "snare", "evacuate"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct CombatRotationScenario {
    class: CombatClass,
    rotations: u64,
}

impl CombatRotationScenario {
    pub fn for_class(class: CombatClass) -> Self {
        Self {
            class,
            rotations: 1,
        }
    }

    pub fn with_rotations(mut self, rotations: u64) -> Self {
        self.rotations = rotations;
        self
    }

    pub fn class(&self) -> CombatClass {
        self.class
    }

    pub fn abilities(&self) -> &'static [&'static str] {
        self.class.abilities()
    }
}

impl TestScenario for CombatRotationScenario {
    fn name(&self) -> &str {
        "combat_rotation"
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async move {
            let abilities_per_rotation = self.abilities().len() as u64;
            let casts = self.rotations.saturating_mul(abilities_per_rotation);

            ScenarioResult::success(duration)
                .with_metric("combat.rotations", MetricValue::Counter(self.rotations))
                .with_metric("combat.ability_casts", MetricValue::Counter(casts))
                .with_metric(
                    "combat.abilities_per_rotation",
                    MetricValue::Counter(abilities_per_rotation),
                )
                .with_metric(
                    "combat.duration_ms",
                    MetricValue::Counter(duration_millis(duration)),
                )
        })
    }
}

pub struct MockScenarios;

impl MockScenarios {
    pub fn countdown(ticks: u64, tick_duration: Duration) -> CountdownScenario {
        CountdownScenario::new(ticks, tick_duration)
    }

    pub fn metric_test() -> MetricTestScenario {
        MetricTestScenario
    }

    pub fn fast_fail(error: impl Into<String>) -> FastFailScenario {
        FastFailScenario::new(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountdownScenario {
    ticks: u64,
    tick_duration: Duration,
}

impl CountdownScenario {
    pub fn new(ticks: u64, tick_duration: Duration) -> Self {
        Self {
            ticks,
            tick_duration,
        }
    }
}

impl TestScenario for CountdownScenario {
    fn name(&self) -> &str {
        "countdown"
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async move {
            let requested_ms =
                u128::from(self.ticks).saturating_mul(self.tick_duration.as_millis());
            let elapsed = duration.min(duration_from_millis(requested_ms));
            let tick_ms = self.tick_duration.as_millis();
            let completed = if tick_ms == 0 {
                self.ticks
            } else {
                self.ticks.min((elapsed.as_millis() / tick_ms) as u64)
            };

            ScenarioResult::success(elapsed)
                .with_metric("countdown.ticks_requested", MetricValue::Counter(self.ticks))
                .with_metric("countdown.ticks_completed", MetricValue::Counter(completed))
                .with_metric(
                    "countdown.duration_ms",
                    MetricValue::Counter(duration_millis(elapsed)),
                )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricTestScenario;

impl TestScenario for MetricTestScenario {
    fn name(&self) -> &str {
        "metric_test"
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async move {
            ScenarioResult::success(duration)
                .with_metric("metric.counter", MetricValue::Counter(7))
                .with_metric("metric.gauge", MetricValue::Gauge(3.5))
                .with_metric("metric.histogram", MetricValue::Histogram(vec![1.0, 2.0, 3.0]))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastFailScenario {
    error: String,
}

impl FastFailScenario {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
        }
    }
}

impl TestScenario for FastFailScenario {
    fn name(&self) -> &str {
        "fast_fail"
    }

    fn run(&mut self, _duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async move {
            ScenarioResult::failure(Duration::ZERO, vec![self.error.clone()])
                .with_metric("fast_fail.errors", MetricValue::Counter(1))
        })
    }
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn duration_from_millis(millis: u128) -> Duration {
    Duration::from_millis(u64::try_from(millis).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests;
