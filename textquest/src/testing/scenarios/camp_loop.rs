//! Camp loop scenario for exercising the camp state machine in tests.

use crate::{
    camp::{
        config::CampConfig,
        state::{CampAction, CampLoop, CampMember, CampSnapshot, CampState, Role},
    },
    testing::scenario::{BoxScenarioFuture, MetricValue, ScenarioResult, TestScenario},
};
use std::time::Duration;

const SCENARIO_NAME: &str = "camp_loop";
const TICK_DURATION: Duration = Duration::from_millis(100);
const MOB_HIT_POINTS: f64 = 1_000.0;
const DAMAGE_PER_TICK: f64 = 250.0;

/// Runs the camp loop state machine against deterministic snapshots.
pub struct CampLoopScenario {
    camp_config: CampConfig,
    max_duration: Duration,
}

impl CampLoopScenario {
    /// Create a camp-loop scenario with the config under test and a hard time limit.
    #[must_use]
    pub fn new(camp_config: CampConfig, max_duration: Duration) -> Self {
        Self {
            camp_config,
            max_duration,
        }
    }

    /// Camp configuration used when constructing the underlying [`CampLoop`].
    #[must_use]
    pub fn camp_config(&self) -> &CampConfig {
        &self.camp_config
    }

    /// Maximum simulated duration this scenario will run for.
    #[must_use]
    pub fn max_duration(&self) -> Duration {
        self.max_duration
    }

    fn run_simulation(&self, duration: Duration) -> ScenarioResult {
        let budget = duration.min(self.max_duration);
        let max_ticks = budget.as_nanos() / TICK_DURATION.as_nanos();
        let mut camp_loop = CampLoop::new(self.camp_config.clone(), default_members());
        let mut sim = CampLoopSimulation::default();

        for _ in 0..max_ticks {
            let previous_state = camp_loop.state.clone();
            let snapshot = sim.snapshot_for(&camp_loop);

            if snapshot.target_is_dead {
                if let Some(spawn_id) = sim.active_spawn_id {
                    camp_loop.record_kill(spawn_id, camp_loop.last_pull_target.clone());
                }
            }

            let _commands = camp_loop.tick(Some(&snapshot));
            sim.observe_transition(&previous_state, &camp_loop.state, &snapshot);
        }

        let elapsed = nanos_duration(TICK_DURATION.as_nanos() * max_ticks);
        sim.into_result(elapsed, cleanup_action_count(&camp_loop))
    }
}

impl TestScenario for CampLoopScenario {
    fn name(&self) -> &str {
        SCENARIO_NAME
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async move { self.run_simulation(duration) })
    }
}

/// Scenario profile for 36-box farm behaviour tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiboxFarmMode {
    /// Baseline full loop with no failure injections.
    FullAutomation,
    /// Include low tank HP windows to exercise healer reaction behaviour.
    GroupCoordination,
    /// Simulate a short member-death window to verify recovery commands resume the farm.
    FailureRecovery,
}

impl MultiboxFarmMode {
    fn name(self) -> &'static str {
        match self {
            Self::FullAutomation => "multibox_farm_full_automation",
            Self::GroupCoordination => "multibox_farm_group_coordination",
            Self::FailureRecovery => "multibox_farm_failure_recovery",
        }
    }
}

/// Deterministic, spawn-aware farm scenario used by integration runner tests.
pub struct MultiboxFarmScenario {
    camp_config: CampConfig,
    max_duration: Duration,
    member_count: usize,
    mode: MultiboxFarmMode,
}

impl MultiboxFarmScenario {
    /// Create a new multibox scenario for a specific member count and farm mode.
    #[must_use]
    pub fn new(
        camp_config: CampConfig,
        max_duration: Duration,
        member_count: usize,
        mode: MultiboxFarmMode,
    ) -> Self {
        Self {
            camp_config,
            max_duration,
            member_count,
            mode,
        }
    }

    fn run_simulation(&self, duration: Duration) -> ScenarioResult {
        let budget = duration.min(self.max_duration);
        let max_ticks = budget.as_nanos() / TICK_DURATION.as_nanos();
        let members = build_multibox_members(self.member_count);
        let mut camp_loop = CampLoop::new(self.camp_config.clone(), members);
        let mut sim = MultiboxFarmSimulation::new(self.mode, self.member_count);

        for tick in 0..max_ticks {
            let current_state = camp_loop.state.clone();
            let snapshot = sim.snapshot_for(tick as u64, &mut camp_loop);
            let commands = camp_loop.tick(Some(&snapshot));

            sim.observe_transition(&current_state, &camp_loop.state, &snapshot);
            sim.observe_commands(&commands);
        }

        let elapsed = nanos_duration(TICK_DURATION.as_nanos() * max_ticks);
        sim.into_result(elapsed, cleanup_action_count(&camp_loop))
    }
}

impl TestScenario for MultiboxFarmScenario {
    fn name(&self) -> &str {
        self.mode.name()
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async move { self.run_simulation(duration) })
    }
}

#[derive(Debug)]
struct MultiboxFarmSimulation {
    mode: MultiboxFarmMode,
    active_spawn_id: Option<u32>,
    next_spawn_id: u32,
    target_hp: f64,
    pulls: u64,
    kills: u64,
    total_damage: f64,
    deaths: u64,
    commands_total: u64,
    attack_commands: u64,
    engage_commands: u64,
    cast_commands: u64,
    cast_rez_commands: u64,
    healer_casts: u64,
    recovery_transitions: u64,
    autoinventory_commands: u64,
    move_commands: u64,
    member_count: usize,
}

impl MultiboxFarmSimulation {
    fn new(mode: MultiboxFarmMode, member_count: usize) -> Self {
        Self {
            mode,
            active_spawn_id: None,
            next_spawn_id: 1,
            target_hp: MOB_HIT_POINTS,
            pulls: 0,
            kills: 0,
            total_damage: 0.0,
            deaths: 0,
            commands_total: 0,
            attack_commands: 0,
            engage_commands: 0,
            cast_commands: 0,
            cast_rez_commands: 0,
            healer_casts: 0,
            recovery_transitions: 0,
            autoinventory_commands: 0,
            move_commands: 0,
            member_count,
        }
    }

    fn snapshot_for(&mut self, tick: u64, camp_loop: &mut CampLoop) -> CampSnapshot {
        let mut target_is_dead = false;
        let in_fight = matches!(camp_loop.state, CampState::Fighting { .. });

        if in_fight {
            self.ensure_target();
            let damage = self.target_hp.min(DAMAGE_PER_TICK);
            self.target_hp -= damage;
            self.total_damage += damage;
            target_is_dead = self.target_hp <= f64::EPSILON;
        }

        if target_is_dead && let Some(spawn_id) = self.active_spawn_id {
            self.kills += 1;
            camp_loop.record_kill(spawn_id, camp_loop.last_pull_target.clone());
            self.active_spawn_id = None;
            self.target_hp = MOB_HIT_POINTS;
        }

        let tank_pid = camp_loop.members.iter().find_map(|member| {
            if matches!(member.role, Role::Tank) {
                Some(member.pid)
            } else {
                None
            }
        });
        let in_combat = matches!(
            camp_loop.state,
            CampState::Pulling { .. } | CampState::Fighting { .. }
        );
        let is_recovery_tick = matches!(camp_loop.state, CampState::Recovery { .. });
        let healer_pressure = matches!(self.mode, MultiboxFarmMode::GroupCoordination)
            && (40..=65).contains(&tick)
            && in_fight
            && !is_recovery_tick;
        let tank_dead = matches!(self.mode, MultiboxFarmMode::FailureRecovery)
            && (20..=45).contains(&tick)
            && in_fight;

        let mut dead_members = 0u64;

        let member_hp: Vec<(u32, i32)> = camp_loop
            .members
            .iter()
            .map(|member| {
                let is_tank = Some(member.pid) == tank_pid;
                let mut hp = 1000;

                if is_tank && tank_dead {
                    hp = 0;
                    dead_members = dead_members.saturating_add(1);
                }
                (member.pid, hp)
            })
            .collect();
        self.deaths = self.deaths.saturating_add(dead_members);
        let member_in_combat = camp_loop
            .members
            .iter()
            .map(|member| (member.pid, in_combat))
            .collect();

        let tank_hp_pct = if healer_pressure { 15.0 } else { 100.0 };
        let target_id = if target_is_dead { None } else { self.active_spawn_id };

        CampSnapshot {
            healer_mana_pct: 100.0,
            tank_hp_pct,
            target_hp_pct: self.target_hp_pct(),
            target_is_dead,
            target_spawn_id: target_id,
            member_hp,
            member_in_combat,
        }
    }

    fn observe_transition(
        &mut self,
        previous_state: &CampState,
        current_state: &CampState,
        _snapshot: &CampSnapshot,
    ) {
        if !matches!(previous_state, CampState::Pulling { .. }) && matches!(current_state, CampState::Pulling { .. })
        {
            self.pulls += 1;
            self.start_next_target();
        }

        if !matches!(previous_state, CampState::Recovery { .. })
            && matches!(current_state, CampState::Recovery { .. })
        {
            self.recovery_transitions += 1;
        }
    }

    fn observe_commands(&mut self, commands: &[(u32, CampAction)]) {
        self.commands_total = self.commands_total.saturating_add(commands.len() as u64);

        for (_, action) in commands {
            match action {
                CampAction::CombatEngage { .. } => {
                    self.engage_commands = self.engage_commands.saturating_add(1);
                }
                CampAction::Slash(cmd) => {
                    self.cast_commands = self.cast_commands.saturating_add(1);
                    if cmd == "/attack" {
                        self.attack_commands = self.attack_commands.saturating_add(1);
                    }
                    if cmd.starts_with("/cast 1") {
                        self.healer_casts = self.healer_casts.saturating_add(1);
                    }
                    if cmd.contains("/rez") {
                        self.cast_rez_commands = self.cast_rez_commands.saturating_add(1);
                    }
                    if cmd == "/autoinventory" {
                        self.autoinventory_commands = self.autoinventory_commands.saturating_add(1);
                    }
                    if cmd.starts_with("/moveto") {
                        self.move_commands = self.move_commands.saturating_add(1);
                    }
                }
            }
        }
    }

    fn ensure_target(&mut self) {
        if self.active_spawn_id.is_none() {
            self.start_next_target();
        }
    }

    fn start_next_target(&mut self) {
        self.active_spawn_id = Some(self.next_spawn_id);
        self.next_spawn_id = self.next_spawn_id.saturating_add(1);
        self.target_hp = MOB_HIT_POINTS;
    }

    fn target_hp_pct(&self) -> Option<f32> {
        self.active_spawn_id
            .map(|_| ((self.target_hp / MOB_HIT_POINTS) * 100.0).max(0.0) as f32)
    }

    fn into_result(self, elapsed: Duration, cleanup_actions: u64) -> ScenarioResult {
        let elapsed_secs = elapsed.as_secs_f64();
        let elapsed_hours = elapsed_secs / 3_600.0;
        let dps = if elapsed_secs > 0.0 {
            self.total_damage / elapsed_secs
        } else {
            0.0
        };
        let pulls_per_hour = if elapsed_hours > 0.0 {
            self.pulls as f64 / elapsed_hours
        } else {
            0.0
        };
        let kill_rate = if self.pulls > 0 {
            self.kills as f64 / self.pulls as f64
        } else {
            0.0
        };

        ScenarioResult::success(elapsed)
            .with_metric("pulls", MetricValue::Counter(self.pulls))
            .with_metric("kills", MetricValue::Counter(self.kills))
            .with_metric("dps", MetricValue::Gauge(dps))
            .with_metric("deaths", MetricValue::Counter(self.deaths))
            .with_metric("pulls_per_hour", MetricValue::Gauge(pulls_per_hour))
            .with_metric("kill_rate", MetricValue::Gauge(kill_rate))
            .with_metric("member_count", MetricValue::Counter(self.member_count as u64))
            .with_metric("commands", MetricValue::Counter(self.commands_total))
            .with_metric("attack_commands", MetricValue::Counter(self.attack_commands))
            .with_metric("engage_commands", MetricValue::Counter(self.engage_commands))
            .with_metric("cast_commands", MetricValue::Counter(self.cast_commands))
            .with_metric("healer_casts", MetricValue::Counter(self.healer_casts))
            .with_metric("rez_casts", MetricValue::Counter(self.cast_rez_commands))
            .with_metric(
                "autoinventory_commands",
                MetricValue::Counter(self.autoinventory_commands),
            )
            .with_metric("move_commands", MetricValue::Counter(self.move_commands))
            .with_metric(
                "recovery_transitions",
                MetricValue::Counter(self.recovery_transitions),
            )
            .with_metric(
                "cleanup_actions",
                MetricValue::Counter(cleanup_actions),
            )
    }
}

fn build_multibox_members(member_count: usize) -> Vec<CampMember> {
    const ROLES: [Role; 7] = [
        Role::Tank,
        Role::Healer,
        Role::Puller,
        Role::Dps,
        Role::Dps,
        Role::CC,
        Role::Bard,
    ];

    let mut members = Vec::with_capacity(member_count);
    for index in 0..member_count {
        let role = ROLES[index % ROLES.len()];
        members.push(CampMember::new(
            1000 + index as u32,
            format!("farm_{index:02}"),
            role,
        ));
    }
    members
}

#[derive(Debug)]
struct CampLoopSimulation {
    active_spawn_id: Option<u32>,
    next_spawn_id: u32,
    target_hp: f64,
    pulls: u64,
    kills: u64,
    total_damage: f64,
    deaths: u64,
}

impl Default for CampLoopSimulation {
    fn default() -> Self {
        Self {
            active_spawn_id: None,
            next_spawn_id: 1,
            target_hp: MOB_HIT_POINTS,
            pulls: 0,
            kills: 0,
            total_damage: 0.0,
            deaths: 0,
        }
    }
}

impl CampLoopSimulation {
    fn snapshot_for(&mut self, camp_loop: &CampLoop) -> CampSnapshot {
        let mut target_is_dead = false;

        if matches!(camp_loop.state, CampState::Fighting { .. }) {
            self.ensure_target();
            let damage = self.target_hp.min(DAMAGE_PER_TICK);
            self.target_hp -= damage;
            self.total_damage += damage;
            target_is_dead = self.target_hp <= f64::EPSILON;
        }

        let in_combat = matches!(
            camp_loop.state,
            CampState::Pulling { .. } | CampState::Fighting { .. }
        );

        CampSnapshot {
            healer_mana_pct: 100.0,
            tank_hp_pct: 100.0,
            target_hp_pct: self.target_hp_pct(),
            target_is_dead,
            target_spawn_id: self.active_spawn_id,
            member_hp: camp_loop.members.iter().map(|m| (m.pid, 1_000)).collect(),
            member_in_combat: camp_loop
                .members
                .iter()
                .map(|m| (m.pid, in_combat))
                .collect(),
        }
    }

    fn observe_transition(
        &mut self,
        previous_state: &CampState,
        current_state: &CampState,
        snapshot: &CampSnapshot,
    ) {
        if !matches!(previous_state, CampState::Pulling { .. })
            && matches!(current_state, CampState::Pulling { .. })
        {
            self.pulls += 1;
            self.start_next_target();
        }

        if matches!(previous_state, CampState::Fighting { .. }) && snapshot.target_is_dead {
            self.kills += 1;
            self.active_spawn_id = None;
            self.target_hp = MOB_HIT_POINTS;
        }

        self.deaths += snapshot.member_hp.iter().filter(|(_, hp)| *hp <= 0).count() as u64;
    }

    fn start_next_target(&mut self) {
        self.active_spawn_id = Some(self.next_spawn_id);
        self.next_spawn_id += 1;
        self.target_hp = MOB_HIT_POINTS;
    }

    fn ensure_target(&mut self) {
        if self.active_spawn_id.is_none() {
            self.start_next_target();
        }
    }

    fn target_hp_pct(&self) -> Option<f32> {
        self.active_spawn_id
            .map(|_| ((self.target_hp / MOB_HIT_POINTS) * 100.0).max(0.0) as f32)
    }

    fn into_result(self, elapsed: Duration, cleanup_actions: u64) -> ScenarioResult {
        let elapsed_secs = elapsed.as_secs_f64();
        let elapsed_hours = elapsed_secs / 3_600.0;
        let dps = if elapsed_secs > 0.0 {
            self.total_damage / elapsed_secs
        } else {
            0.0
        };
        let pulls_per_hour = if elapsed_hours > 0.0 {
            self.pulls as f64 / elapsed_hours
        } else {
            0.0
        };
        let kill_rate = if self.pulls > 0 {
            self.kills as f64 / self.pulls as f64
        } else {
            0.0
        };

        ScenarioResult::success(elapsed)
            .with_metric("pulls", MetricValue::Counter(self.pulls))
            .with_metric("kills", MetricValue::Counter(self.kills))
            .with_metric("dps", MetricValue::Gauge(dps))
            .with_metric("deaths", MetricValue::Counter(self.deaths))
            .with_metric("pulls_per_hour", MetricValue::Gauge(pulls_per_hour))
            .with_metric("kill_rate", MetricValue::Gauge(kill_rate))
            .with_metric("cleanup_actions", MetricValue::Counter(cleanup_actions))
    }
}

fn default_members() -> Vec<CampMember> {
    vec![
        CampMember::new(100, "Tank".into(), Role::Tank),
        CampMember::new(101, "Cleric".into(), Role::Healer),
        CampMember::new(102, "Rogue".into(), Role::Dps),
        CampMember::new(103, "Puller".into(), Role::Puller),
    ]
}

fn cleanup_action_count(camp_loop: &CampLoop) -> u64 {
    match &camp_loop.state {
        CampState::Pulling { .. } | CampState::Fighting { .. } | CampState::Recovery { .. } => {
            camp_loop.members.len() as u64 * 2
        }
        _ => 0,
    }
}

fn nanos_duration(nanos: u128) -> Duration {
    let secs = (nanos / 1_000_000_000) as u64;
    let subsec_nanos = (nanos % 1_000_000_000) as u32;
    Duration::new(secs, subsec_nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_config() -> CampConfig {
        CampConfig {
            name: "mock_camp".into(),
            zone: "test".into(),
            camp_center: [0.0, 0.0, 0.0],
            pull_point: [10.0, 0.0, 0.0],
            pull_radius: 100.0,
            camp_radius: 30.0,
            leash_radius: 150.0,
            rest_mana_pct: 50,
            pull_mana_pct: 20,
            level_range: [1, 10],
            pull_mob_names: vec!["a test mob".into()],
            ignore_mob_names: Vec::new(),
            burn_mob_names: Vec::new(),
            return_no_aggro: false,
            next_camp: None,
            prev_camp: None,
        }
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

    #[test]
    fn initializes_with_camp_config() {
        let scenario = CampLoopScenario::new(mock_config(), Duration::from_secs(5));

        assert_eq!(scenario.name(), "camp_loop");
        assert_eq!(scenario.camp_config().name, "mock_camp");
        assert_eq!(scenario.max_duration(), Duration::from_secs(5));
    }

    #[tokio::test]
    async fn collects_metrics_during_execution() {
        let mut scenario = CampLoopScenario::new(mock_config(), Duration::from_secs(5));

        let result = scenario.run(Duration::from_secs(2)).await;

        assert!(result.success);
        assert!(counter(&result, "pulls") >= 1);
        assert!(counter(&result, "kills") >= 1);
        assert!(gauge(&result, "dps") > 0.0);
        assert_eq!(counter(&result, "deaths"), 0);
        assert!(gauge(&result, "pulls_per_hour") > 0.0);
        assert!(gauge(&result, "kill_rate") > 0.0);
    }

    #[tokio::test]
    async fn honors_early_termination() {
        let mut scenario = CampLoopScenario::new(mock_config(), Duration::from_secs(5));

        let result = scenario.run(Duration::from_millis(100)).await;

        assert!(result.success);
        assert_eq!(result.duration, Duration::from_millis(100));
        assert_eq!(counter(&result, "pulls"), 1);
        assert_eq!(counter(&result, "kills"), 0);
        assert_eq!(gauge(&result, "kill_rate"), 0.0);
    }

    #[tokio::test]
    async fn supports_multiple_iterations_with_mock_camp_config() {
        let mut scenario = CampLoopScenario::new(mock_config(), Duration::from_secs(8));

        let result = scenario.run(Duration::from_secs(8)).await;

        assert!(result.success);
        assert!(counter(&result, "pulls") >= 2);
        assert!(counter(&result, "kills") >= 2);
        assert!(gauge(&result, "kill_rate") <= 1.0);
    }
}
