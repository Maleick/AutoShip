//! Combat rotation scenario for class validation and DPS measurement.

use std::time::{Duration, Instant};

use crate::{
    combat::{
        class_strategy::{ClassConfig, ClassStrategy, CombatAction, StaticClassStrategy},
        events::{CombatEvent, CombatEventBuffer, DamageType},
        state::Combatant,
    },
    testing::scenario::{BoxScenarioFuture, MetricValue, ScenarioResult, TestScenario},
};

/// Scenario that executes a deterministic combat rotation against a target.
#[derive(Debug, Clone)]
pub struct CombatRotationScenario {
    target_spawn: String,
    class_config: ClassConfig,
    max_pulls: u32,
}

impl CombatRotationScenario {
    /// Construct a combat rotation scenario.
    #[must_use]
    pub fn new(target_spawn: impl Into<String>, class_config: ClassConfig, max_pulls: u32) -> Self {
        Self {
            target_spawn: target_spawn.into(),
            class_config,
            max_pulls,
        }
    }

    fn execute(&mut self, duration: Duration) -> ScenarioResult {
        if self.target_spawn.trim().is_empty() {
            return ScenarioResult::failure(duration, vec!["target_spawn must not be empty".into()]);
        }
        if self.max_pulls == 0 {
            return ScenarioResult::failure(duration, vec!["max_pulls must be greater than zero".into()]);
        }

        let strategy = StaticClassStrategy::new(self.class_config.clone());
        if let Err(error) = strategy.validate() {
            return ScenarioResult::failure(duration, vec![error]);
        }

        let mut combatant = Combatant::new(strategy.config().source_id);
        let mut events = CombatEventBuffer::new();
        let start = Instant::now();
        let mut simulated_elapsed = Duration::ZERO;
        let mut rotation_times = Vec::new();
        let mut action_attempts = 0_u32;
        let target_id = target_id_for_spawn(&self.target_spawn);

        while rotation_times.len() < self.max_pulls as usize {
            let rotation_duration = strategy.config().rotation_duration();
            if simulated_elapsed + rotation_duration > duration {
                break;
            }

            let rotation_start = simulated_elapsed;
            combatant.start_pull(self.target_spawn.clone());
            combatant.start_rotation(self.target_spawn.clone());

            for action in strategy.rotation() {
                action_attempts += 1;
                combatant.start_action(action);
                simulated_elapsed += action.duration();

                if should_interrupt(strategy.config(), action_attempts) {
                    combatant.interrupt_action();
                    continue;
                }

                combatant.complete_action(self.target_spawn.clone());
                events.push(combat_event(
                    start + simulated_elapsed,
                    combatant.source_id,
                    target_id,
                    action,
                ));
            }

            combatant.complete_pull();
            rotation_times.push((simulated_elapsed - rotation_start).as_secs_f64());
        }

        let event_snapshot: Vec<_> = events.events_since(start).collect();
        let damage: u64 = event_snapshot
            .iter()
            .map(|event| u64::from(event.damage))
            .sum();
        let total_hits = event_snapshot.len() as u64;
        let critical_hits = event_snapshot
            .iter()
            .filter(|event| event.is_critical)
            .count() as u64;
        let spell_casts = event_snapshot
            .iter()
            .filter(|event| event.damage_type == DamageType::Spell)
            .count() as u64;
        let melee_hits = event_snapshot
            .iter()
            .filter(|event| event.damage_type == DamageType::Melee)
            .count() as u64;
        let interrupt_count = interrupt_count(strategy.config(), action_attempts);
        let dps = if simulated_elapsed.is_zero() {
            0.0
        } else {
            damage as f64 / simulated_elapsed.as_secs_f64()
        };
        let crit_rate = if total_hits == 0 {
            0.0
        } else {
            critical_hits as f64 / total_hits as f64
        };
        let avg_rotation_time = if rotation_times.is_empty() {
            0.0
        } else {
            rotation_times.iter().sum::<f64>() / rotation_times.len() as f64
        };

        ScenarioResult::success(simulated_elapsed)
            .with_metric(
                "rotations_completed",
                MetricValue::Counter(rotation_times.len() as u64),
            )
            .with_metric("spell_casts", MetricValue::Counter(spell_casts))
            .with_metric("melee_hits", MetricValue::Counter(melee_hits))
            .with_metric("dps", MetricValue::Gauge(dps))
            .with_metric("interrupt_count", MetricValue::Counter(interrupt_count))
            .with_metric("crit_rate", MetricValue::Gauge(crit_rate))
            .with_metric("avg_rotation_time", MetricValue::Gauge(avg_rotation_time))
    }
}

impl TestScenario for CombatRotationScenario {
    fn name(&self) -> &str {
        "combat_rotation"
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async move { self.execute(duration) })
    }
}

fn should_interrupt(config: &ClassConfig, action_attempt: u32) -> bool {
    config
        .interrupt_every
        .is_some_and(|cadence| action_attempt % cadence == 0)
}

fn interrupt_count(config: &ClassConfig, action_attempts: u32) -> u64 {
    config
        .interrupt_every
        .map(|cadence| u64::from(action_attempts / cadence))
        .unwrap_or(0)
}

fn combat_event(
    timestamp: Instant,
    source_id: u32,
    target_id: u32,
    action: &CombatAction,
) -> CombatEvent {
    CombatEvent {
        timestamp,
        source_id,
        target_id,
        damage: action.damage(),
        damage_type: if action.is_spell() {
            DamageType::Spell
        } else {
            DamageType::Melee
        },
        spell_name: action.spell_name().map(str::to_string),
        is_critical: action.is_critical(),
        is_kill: false,
    }
}

fn target_id_for_spawn(target_spawn: &str) -> u32 {
    let hash = target_spawn.bytes().fold(0x811c_9dc5_u32, |acc, byte| {
        acc.wrapping_mul(16_777_619) ^ u32::from(byte)
    });
    hash.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn single_rotation_collects_spell_metrics() {
        let config = ClassConfig::new(
            "Wizard",
            7,
            vec![
                CombatAction::spell("Nuke", 100, Duration::from_secs(1), false),
                CombatAction::spell("Bolt", 50, Duration::from_secs(1), true),
            ],
        );
        let mut scenario = CombatRotationScenario::new("a test dummy", config, 1);

        let result = scenario.run(Duration::from_secs(5)).await;

        assert!(result.success);
        assert_eq!(counter(&result, "rotations_completed"), 1);
        assert_eq!(counter(&result, "spell_casts"), 2);
        assert_eq!(counter(&result, "melee_hits"), 0);
        assert_eq!(gauge(&result, "avg_rotation_time"), 2.0);
        assert_eq!(gauge(&result, "crit_rate"), 0.5);
    }

    #[tokio::test]
    async fn multiple_rotations_respect_max_pulls() {
        let config = ClassConfig::new(
            "Warrior",
            8,
            vec![CombatAction::melee(40, Duration::from_secs(1), false)],
        );
        let mut scenario = CombatRotationScenario::new("a test dummy", config, 3);

        let result = scenario.run(Duration::from_secs(10)).await;

        assert!(result.success);
        assert_eq!(counter(&result, "rotations_completed"), 3);
        assert_eq!(counter(&result, "melee_hits"), 3);
        assert_eq!(counter(&result, "spell_casts"), 0);
    }

    #[tokio::test]
    async fn interrupts_skip_interrupted_damage() {
        let config = ClassConfig::new(
            "Wizard",
            9,
            vec![
                CombatAction::spell("Nuke", 100, Duration::from_secs(1), false),
                CombatAction::spell("Bolt", 100, Duration::from_secs(1), false),
            ],
        )
        .with_interrupt_every(Some(2));
        let mut scenario = CombatRotationScenario::new("a test dummy", config, 1);

        let result = scenario.run(Duration::from_secs(5)).await;

        assert!(result.success);
        assert_eq!(counter(&result, "rotations_completed"), 1);
        assert_eq!(counter(&result, "interrupt_count"), 1);
        assert_eq!(counter(&result, "spell_casts"), 1);
        assert_eq!(gauge(&result, "dps"), 50.0);
    }

    #[tokio::test]
    async fn dps_uses_damage_over_simulated_duration() {
        let config = ClassConfig::new(
            "Wizard",
            10,
            vec![CombatAction::spell(
                "Measured Nuke",
                120,
                Duration::from_secs(3),
                false,
            )],
        );
        let mut scenario = CombatRotationScenario::new("a test dummy", config, 1);

        let result = scenario.run(Duration::from_secs(3)).await;

        assert!(result.success);
        assert_eq!(gauge(&result, "dps"), 40.0);
    }

    #[tokio::test]
    async fn per_class_validation_rejects_invalid_rotation() {
        let config = ClassConfig::new(
            "Warrior",
            11,
            vec![CombatAction::spell("Wrong", 100, Duration::from_secs(1), false)],
        );
        let mut scenario = CombatRotationScenario::new("a test dummy", config, 1);

        let result = scenario.run(Duration::from_secs(5)).await;

        assert!(!result.success);
        assert!(result.errors[0].contains("must include melee damage"));
    }
}
