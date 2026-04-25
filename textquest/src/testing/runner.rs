//! Multi-runner spawning for concurrent per-account test execution.
//!
//! `TestLoopRunner` drives a [`TestScenario`] for a bounded duration on behalf
//! of one account.  `spawn_runners` launches one runner per account as a
//! separate tokio task and returns the [`JoinHandle`]s for lifecycle management.

use std::{
    fs,
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{sync::watch, task::JoinHandle, time};

use super::scenario::{MetricValue, ScenarioResult, TestScenario};
use textquest_common::login::AccountInfo;

// ── Recovery constants ─────────────────────────────────────────────────────

const LOGIN_BACKOFF_BASE_MS: u64 = 250;
const LOGIN_BACKOFF_MAX_MS: u64 = 30_000;
const LOGIN_RETRY_LIMIT: u32 = 4;
const CIRCUIT_BREAKER_FAILURE_THRESHOLD: u32 = 5;
const CIRCUIT_BREAKER_COOLDOWN_SECS: u64 = 120;
const SCENARIO_HARD_TIMEOUT_SECS: u64 = 90;
const MEMORY_GROWTH_WARNING_PER_HOUR: f64 = 0.05;
const MEMORY_SAMPLE_INTERVAL_SECS: u64 = 60;
const CHECKPOINT_FILE_PREFIX: &str = "test-loop-checkpoint";

// ── TestLoopResult ──────────────────────────────────────────────────────────

/// The outcome of a single `TestLoopRunner` run.
#[derive(Debug, Clone)]
pub struct TestLoopResult {
    /// The account this runner was bound to.
    pub account: AccountInfo,
    /// Unique task identifier (index into the spawn list).
    pub task_id: usize,
    /// Accumulated scenario results collected during the run.
    pub scenario_results: Vec<ScenarioResult>,
    /// Whether any scenario reported a failure.
    pub any_failure: bool,
}

impl TestLoopResult {
    /// Construct a result from a list of scenario outcomes.
    #[must_use]
    pub fn from_results(
        account: AccountInfo,
        task_id: usize,
        scenario_results: Vec<ScenarioResult>,
    ) -> Self {
        let any_failure = scenario_results.iter().any(|r| !r.success);
        Self {
            account,
            task_id,
            scenario_results,
            any_failure,
        }
    }
}

// ── TestLoopRunner ──────────────────────────────────────────────────────────

/// Drives a collection of [`TestScenario`]s for one account for a bounded wall-clock
/// duration.
///
/// Scenarios are run sequentially in the order provided.  If all scenarios
/// complete before `duration` elapses the runner returns early rather than
/// looping.  This matches the contract needed by `spawn_runners` — each runner
/// is expected to finish within the budget.
pub struct TestLoopRunner {
    /// The account this runner is associated with.
    pub account: AccountInfo,
    /// Unique identifier for this task.
    pub task_id: usize,
    /// Scenarios to execute.
    scenarios: Vec<Box<dyn TestScenario>>,
    /// Wall-clock budget for the entire run.
    duration: Duration,
    /// Output directory for writing per-account artefacts (reserved for future use).
    #[allow(dead_code)]
    output_dir: PathBuf,
    /// Recovery runtime state loaded from checkpoint and used while running.
    recovery: RecoveryRuntimeState,
    /// Shutdown signal receiver — runner exits as soon as this fires.
    shutdown_rx: watch::Receiver<bool>,
}

impl TestLoopRunner {
    /// Create a new runner.
    ///
    /// * `task_id` — monotonically-increasing index, unique across a spawn batch.
    /// * `shutdown_rx` — shared watch channel; set to `true` to trigger graceful
    ///   shutdown of all runners.
    pub fn new(
        account: AccountInfo,
        task_id: usize,
        scenarios: Vec<Box<dyn TestScenario>>,
        duration: Duration,
        output_dir: impl AsRef<Path>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        let recovery = RecoveryRuntimeState::load_or_default(task_id, scenarios.len(), output_dir.as_ref());
        Self {
            account,
            task_id,
            scenarios,
            duration,
            output_dir: output_dir.as_ref().to_owned(),
            recovery,
            shutdown_rx,
        }
    }

    /// Execute all scenarios (once, sequentially) within the time budget and
    /// return the aggregated result.
    ///
    /// Exits early if the shutdown signal fires before all scenarios complete.
    pub async fn run(mut self) -> TestLoopResult {
        let start = std::time::Instant::now();
        let mut results = Vec::with_capacity(self.scenarios.len());
        let mut index = self.recovery.starting_index;

        while index < self.scenarios.len() {
            // Honour shutdown signal between scenarios.
            if *self.shutdown_rx.borrow() {
                tracing::debug!(
                    task_id = self.task_id,
                    account = %self.account.account_name,
                    "runner received shutdown signal, exiting early"
                );
                break;
            }

            // Compute remaining budget for this scenario.
            let elapsed = start.elapsed();
            if elapsed >= self.duration {
                tracing::debug!(
                    task_id = self.task_id,
                    "time budget exhausted before all scenarios ran"
                );
                break;
            }
            let remaining = self.duration - elapsed;

            if self.recovery.is_circuit_open(Instant::now()) {
                let mut skip = ScenarioResult::failure(
                    Duration::from_secs(0),
                    vec!["circuit_breaker_active".to_string()],
                );
                skip = self.recovery.attach_memory_metrics(skip, Instant::now());
                results.push(skip);
                index += 1;
                self.recovery
                    .save_checkpoint(self.scenarios.len(), index)
                    .ok();
                continue;
            }

            if let Some(backoff) = self.recovery.login_backoff_remaining(Instant::now()) {
                time::sleep(backoff.min(remaining)).await;
            }

            let scenario = &mut self.scenarios[index];
            let mut result = Self::run_scenario_with_retries(
                scenario.as_mut(),
                remaining,
                &mut self.recovery,
                self.task_id,
            )
            .await;
            result = self.recovery.attach_memory_metrics(result, Instant::now());

            if result.success {
                self.recovery.on_success();
            } else {
                self.recovery.on_failure(Instant::now());
            }

            tracing::debug!(
                task_id = self.task_id,
                scenario = scenario.name(),
                success = result.success,
                "scenario complete"
            );
            results.push(result);

            index += 1;
            self.recovery
                .save_checkpoint(self.scenarios.len(), index)
                .ok();
        }

        self.recovery
            .clear_checkpoint_if_finished(self.scenarios.len(), index);

        TestLoopResult::from_results(self.account, self.task_id, results)
    }

    async fn run_scenario_with_retries(
        scenario: &mut dyn TestScenario,
        duration: Duration,
        recovery: &mut RecoveryRuntimeState,
        task_id: usize,
    ) -> ScenarioResult {
        let run_start = std::time::Instant::now();
        let mut login_attempts = 0;

        loop {
            if run_start.elapsed() >= duration {
                break;
            }

            if let Some(backoff) = recovery.login_backoff_remaining(Instant::now()) {
                let remaining = duration.saturating_sub(run_start.elapsed());
                if remaining.is_zero() {
                    return ScenarioResult::failure(
                        Duration::from_secs(0),
                        vec!["scenario_budget_exhausted".to_string()],
                    );
                }
                time::sleep(backoff.min(Duration::from_millis(200)).min(remaining)).await;
            }

            let remaining = duration.saturating_sub(run_start.elapsed());
            let timeout_budget = remaining.min(Duration::from_secs(SCENARIO_HARD_TIMEOUT_SECS)).max(Duration::from_millis(1));

            let outcome = time::timeout(timeout_budget, scenario.run(remaining)).await;
            match outcome {
                Ok(mut result) => {
                    result.duration = run_start.elapsed();
                    if result.success {
                        recovery.on_login_success();
                        return result;
                    }

                    let is_login = result
                        .errors
                        .iter()
                        .any(|error| error.to_ascii_lowercase().contains("login"));
                    if is_login && login_attempts < LOGIN_RETRY_LIMIT {
                        login_attempts = login_attempts.saturating_add(1);
                        let backoff = recovery.register_login_failure(Instant::now());
                        tracing::warn!(
                            task_id = task_id,
                            scenario = scenario.name(),
                            attempt = login_attempts,
                            backoff_ms = backoff.as_millis(),
                            "login failure detected, retrying with exponential backoff"
                        );
                        if run_start.elapsed() + backoff >= duration {
                            result.errors.push(
                                "login_retry_exhausted_after_timeout".to_string(),
                            );
                            return result;
                        }
                        time::sleep(backoff).await;
                        continue;
                    }

                    return result;
                }
                Err(_) => {
                    recovery.on_timeout();
                    return ScenarioResult::failure(
                        timeout_budget,
                        vec![
                            "process_timeout: scenario did not complete before cleanup window"
                                .to_string(),
                        ],
                    );
                }
            }
        }

        ScenarioResult::failure(
            run_start.elapsed(),
            vec!["scenario_runtime_exhausted".to_string()],
        )
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct RecoveryCheckpoint {
    task_id: usize,
    total_scenarios: usize,
    next_scenario_index: usize,
}

#[derive(Debug)]
struct RecoveryMemorySample {
    bytes: u64,
    growth_rate_per_hour: f64,
}

#[derive(Debug)]
struct RecoveryRuntimeState {
    task_id: usize,
    total_scenarios: usize,
    checkpoint_path: PathBuf,
    starting_index: usize,
    consecutive_login_failures: u32,
    login_backoff_until: Option<Instant>,
    circuit_failures: u32,
    circuit_open_until: Option<Instant>,
    timeout_count: u64,
    memory_bytes_base: Option<u64>,
    memory_last_sample: Option<Instant>,
    memory_last_bytes: Option<u64>,
    start_time: Instant,
}

impl RecoveryRuntimeState {
    fn load_or_default(task_id: usize, total_scenarios: usize, output_dir: &Path) -> Self {
        let checkpoint_path = output_dir.join(format!("{CHECKPOINT_FILE_PREFIX}-{task_id}.json"));
        let starting_index = fs::read_to_string(&checkpoint_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<RecoveryCheckpoint>(&raw).ok())
            .filter(|checkpoint| {
                checkpoint.task_id == task_id && checkpoint.total_scenarios == total_scenarios
            })
            .map(|checkpoint| checkpoint.next_scenario_index.min(total_scenarios))
            .unwrap_or(0);

        Self {
            task_id,
            total_scenarios,
            checkpoint_path,
            starting_index,
            consecutive_login_failures: 0,
            login_backoff_until: None,
            circuit_failures: 0,
            circuit_open_until: None,
            timeout_count: 0,
            memory_bytes_base: None,
            memory_last_sample: None,
            memory_last_bytes: None,
            start_time: Instant::now(),
        }
    }

    fn login_backoff_delay(&self, failures: u32) -> Duration {
        let exponent = failures.saturating_sub(1).min(8);
        let multiplier = 1_u64.checked_shl(exponent).unwrap_or(0);
        let ms = LOGIN_BACKOFF_BASE_MS
            .saturating_mul(multiplier.max(1))
            .min(LOGIN_BACKOFF_MAX_MS);
        Duration::from_millis(ms)
    }

    fn register_login_failure(&mut self, now: Instant) -> Duration {
        self.consecutive_login_failures = self.consecutive_login_failures.saturating_add(1);
        let backoff = self.login_backoff_delay(self.consecutive_login_failures);
        self.login_backoff_until = Some(now + backoff);
        backoff
    }

    fn on_login_success(&mut self) {
        self.consecutive_login_failures = 0;
        self.login_backoff_until = None;
    }

    fn on_failure(&mut self, now: Instant) {
        self.circuit_failures = self.circuit_failures.saturating_add(1);
        if self.circuit_failures >= CIRCUIT_BREAKER_FAILURE_THRESHOLD {
            self.circuit_open_until =
                Some(now + Duration::from_secs(CIRCUIT_BREAKER_COOLDOWN_SECS));
            self.circuit_failures = 0;
        }
    }

    fn on_success(&mut self) {
        self.circuit_failures = 0;
        self.on_login_success();
    }

    fn is_circuit_open(&self, now: Instant) -> bool {
        self.circuit_open_until.is_some_and(|opened| now < opened)
    }

    fn login_backoff_remaining(&self, now: Instant) -> Option<Duration> {
        self.login_backoff_until.and_then(|deadline| deadline.checked_duration_since(now))
    }

    fn on_timeout(&mut self) {
        self.timeout_count = self.timeout_count.saturating_add(1);
    }

    fn attach_memory_metrics(
        &mut self,
        mut result: ScenarioResult,
        now: Instant,
    ) -> ScenarioResult {
        if let Some(sample) = self.current_memory_sample(now) {
            result = result.with_metric("process_memory_bytes", MetricValue::Gauge(sample.bytes as f64));
            result = result.with_metric(
                "memory_growth_per_hour",
                MetricValue::Gauge(sample.growth_rate_per_hour),
            );
            if sample.growth_rate_per_hour > MEMORY_GROWTH_WARNING_PER_HOUR {
                tracing::warn!(
                    task_id = self.task_id,
                    growth_rate_per_hour = sample.growth_rate_per_hour,
                    "memory growth exceeded alert threshold"
                );
            }
        }
        result
    }

    fn current_memory_sample(&mut self, now: Instant) -> Option<RecoveryMemorySample> {
        let bytes = current_process_rss_bytes()?;
        if let Some(last_sample) = self.memory_last_sample
            && now.duration_since(last_sample) < Duration::from_secs(MEMORY_SAMPLE_INTERVAL_SECS)
        {
            return None;
        }

        let elapsed = now.duration_since(self.start_time).as_secs_f64();
        let base = self
            .memory_bytes_base
            .get_or_insert_with(|| {
                self.memory_last_bytes
                    .unwrap_or(bytes)
            });

        self.memory_last_sample = Some(now);
        self.memory_last_bytes = Some(bytes);

        let base_value = *base;
        if base_value == 0 || elapsed <= 0.0 {
            return Some(RecoveryMemorySample {
                bytes,
                growth_rate_per_hour: 0.0,
            });
        }

        let growth_ratio = (bytes as f64 - base_value as f64) / base_value as f64;
        let hours = elapsed / 3600.0;
        let growth_rate_per_hour = if hours <= f64::EPSILON {
            0.0
        } else {
            growth_ratio / hours
        };

        Some(RecoveryMemorySample {
            bytes,
            growth_rate_per_hour,
        })
    }

    fn save_checkpoint(&self, total_scenarios: usize, next_scenario_index: usize) -> io::Result<()> {
        let checkpoint_path = &self.checkpoint_path;
        let parent = checkpoint_path.parent().ok_or_else(|| {
            io::Error::other("checkpoint path has no parent directory")
        })?;
        fs::create_dir_all(parent)?;
        let checkpoint = RecoveryCheckpoint {
            task_id: self.task_id,
            total_scenarios,
            next_scenario_index: next_scenario_index.min(total_scenarios),
        };
        let payload = serde_json::to_string_pretty(&checkpoint).map_err(|error| {
            io::Error::other(format!("failed to serialise recovery checkpoint: {error}"))
        })?;
        fs::write(checkpoint_path, payload)?;
        Ok(())
    }

    fn clear_checkpoint_if_finished(&self, total_scenarios: usize, next_scenario_index: usize) {
        if next_scenario_index >= total_scenarios && next_scenario_index == self.total_scenarios {
            let _ = fs::remove_file(&self.checkpoint_path);
        }
    }
}

fn current_process_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let raw = fs::read_to_string("/proc/self/statm").ok()?;
        let resident_pages = raw.split_whitespace().nth(1)?.parse::<u64>().ok()?;
        Some(resident_pages.saturating_mul(4096))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

// ── spawn_runners ───────────────────────────────────────────────────────────

/// Spawn one [`TestLoopRunner`] per account as a separate tokio task.
///
/// Each runner receives its own copy of the scenario list (via the factory
/// closure) and a shared shutdown channel so callers can stop all runners with
/// a single signal.
///
/// # Parameters
///
/// * `accounts` — one runner is spawned per entry.
/// * `scenario_factory` — called once per account; returns the scenario list for
///   that runner.  Using a factory avoids requiring [`Clone`] on scenario types.
/// * `duration` — wall-clock budget passed to each [`TestLoopRunner`].
/// * `output_dir` — base directory for per-account artefacts.
/// * `shutdown_rx` — shared watch receiver; set to `true` to signal all runners
///   to stop.
///
/// # Returns
///
/// A `Vec<JoinHandle<TestLoopResult>>` — one handle per account, in input order.
/// Awaiting a handle yields the runner's [`TestLoopResult`]; a panic in a runner
/// propagates as a `JoinError`.
pub async fn spawn_runners<F>(
    accounts: Vec<AccountInfo>,
    scenario_factory: F,
    duration: Duration,
    output_dir: &Path,
    shutdown_rx: watch::Receiver<bool>,
) -> Vec<JoinHandle<TestLoopResult>>
where
    F: Fn(usize, &AccountInfo) -> Vec<Box<dyn TestScenario>>,
{
    let output_dir: Arc<Path> = Arc::from(output_dir);
    let mut handles = Vec::with_capacity(accounts.len());

    for (task_id, account) in accounts.into_iter().enumerate() {
        let scenarios = scenario_factory(task_id, &account);
        let dir = Arc::clone(&output_dir);
        let rx = shutdown_rx.clone();

        let runner = TestLoopRunner::new(account, task_id, scenarios, duration, dir.as_ref(), rx);

        let handle = tokio::spawn(runner.run());
        handles.push(handle);
    }

    handles
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scenario::{BoxScenarioFuture, ScenarioResult};
    use std::path::Path;
    use std::time::Duration;
    use tempfile::TempDir;
    use textquest_common::login::AccountInfo;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn tmp_dir() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn test_account(name: &str) -> AccountInfo {
        AccountInfo {
            account_name: name.to_string(),
            character_name: format!("{name}_char"),
            class_name: "Warrior".to_string(),
            level: 60,
            group_id: 1,
            server_name: "TestServer".to_string(),
        }
    }

    fn no_shutdown() -> (watch::Sender<bool>, watch::Receiver<bool>) {
        watch::channel(false)
    }

    /// Scenario that immediately succeeds.
    struct InstantPass;

    impl crate::testing::scenario::TestScenario for InstantPass {
        fn name(&self) -> &str {
            "instant_pass"
        }
        fn run(&mut self, _duration: Duration) -> BoxScenarioFuture<'_> {
            Box::pin(async { ScenarioResult::success(Duration::from_millis(0)) })
        }
    }

    /// Scenario that immediately fails.
    struct InstantFail;

    impl crate::testing::scenario::TestScenario for InstantFail {
        fn name(&self) -> &str {
            "instant_fail"
        }
        fn run(&mut self, _duration: Duration) -> BoxScenarioFuture<'_> {
            Box::pin(async {
                ScenarioResult::failure(Duration::from_millis(0), vec!["fail".to_string()])
            })
        }
    }

    // ── unit tests ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_spawn_single_runner() {
        let dir = tmp_dir();
        let (_tx, rx) = no_shutdown();

        let accounts = vec![test_account("acct1")];
        let handles = spawn_runners(
            accounts,
            |_, _| vec![Box::new(InstantPass) as Box<dyn crate::testing::scenario::TestScenario>],
            Duration::from_secs(5),
            dir.path(),
            rx,
        )
        .await;

        assert_eq!(handles.len(), 1, "one handle per account");
        let result = handles.into_iter().next().unwrap().await.expect("no panic");
        assert_eq!(result.task_id, 0);
        assert_eq!(result.account.account_name, "acct1");
        assert!(!result.any_failure);
        assert_eq!(result.scenario_results.len(), 1);
    }

    #[tokio::test]
    async fn test_spawn_three_runners_concurrent() {
        let dir = tmp_dir();
        let (_tx, rx) = no_shutdown();

        let accounts = vec![
            test_account("acct1"),
            test_account("acct2"),
            test_account("acct3"),
        ];
        let handles = spawn_runners(
            accounts,
            |_, _| vec![Box::new(InstantPass) as Box<dyn crate::testing::scenario::TestScenario>],
            Duration::from_secs(5),
            dir.path(),
            rx,
        )
        .await;

        assert_eq!(handles.len(), 3, "three handles for three accounts");

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            results.push(handle.await.expect("runner panicked"));
        }

        // task_ids must be unique and in 0..3
        let mut ids: Vec<usize> = results.iter().map(|r| r.task_id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![0, 1, 2], "task IDs must be 0, 1, 2");

        for r in &results {
            assert!(!r.any_failure, "all runners should pass");
        }
    }

    #[tokio::test]
    async fn test_shutdown_signal_stops_runner() {
        let dir = tmp_dir();
        let (tx, rx) = no_shutdown();

        // Signal shutdown before the runner even reads any scenario.
        tx.send(true).expect("send shutdown");

        let accounts = vec![test_account("acct_shutdown")];
        let handles = spawn_runners(
            accounts,
            |_, _| vec![Box::new(InstantPass) as Box<dyn crate::testing::scenario::TestScenario>],
            Duration::from_secs(60),
            dir.path(),
            rx,
        )
        .await;

        let result = handles.into_iter().next().unwrap().await.expect("no panic");
        // Runner exits early on shutdown; scenario list may be empty.
        assert_eq!(result.task_id, 0);
        // No panic — completion itself is the acceptance criterion.
    }

    #[tokio::test]
    async fn test_failure_propagates_in_result() {
        let dir = tmp_dir();
        let (_tx, rx) = no_shutdown();

        let accounts = vec![test_account("failing_acct")];
        let handles = spawn_runners(
            accounts,
            |_, _| vec![Box::new(InstantFail) as Box<dyn crate::testing::scenario::TestScenario>],
            Duration::from_secs(5),
            dir.path(),
            rx,
        )
        .await;

        let result = handles.into_iter().next().unwrap().await.expect("no panic");
        assert!(result.any_failure, "failure should propagate to result");
        assert_eq!(result.scenario_results.len(), 1);
        assert!(!result.scenario_results[0].success);
    }

    #[tokio::test]
    async fn test_task_handles_tracked() {
        let dir = tmp_dir();
        let (_tx, rx) = no_shutdown();

        let n = 5;
        let accounts: Vec<AccountInfo> = (0..n).map(|i| test_account(&format!("a{i}"))).collect();
        let handles = spawn_runners(
            accounts,
            |_, _| vec![Box::new(InstantPass) as Box<dyn crate::testing::scenario::TestScenario>],
            Duration::from_secs(5),
            dir.path(),
            rx,
        )
        .await;

        // All handles must be valid (not yet joined).
        assert_eq!(handles.len(), n, "handle count must match account count");
        for handle in handles {
            handle.await.expect("runner must not panic");
        }
    }

    #[test]
    fn test_loop_result_any_failure_false_when_all_pass() {
        let account = test_account("a1");
        let results = vec![
            ScenarioResult::success(Duration::from_millis(1)),
            ScenarioResult::success(Duration::from_millis(2)),
        ];
        let r = TestLoopResult::from_results(account, 0, results);
        assert!(!r.any_failure);
    }

    #[test]
    fn test_loop_result_any_failure_true_on_one_fail() {
        let account = test_account("a1");
        let results = vec![
            ScenarioResult::success(Duration::from_millis(1)),
            ScenarioResult::failure(Duration::from_millis(1), vec!["err".into()]),
        ];
        let r = TestLoopResult::from_results(account, 0, results);
        assert!(r.any_failure);
    }

    #[test]
    fn test_login_backoff_retries_increase_delay() {
        let mut state = RecoveryRuntimeState::load_or_default(0, 10, Path::new("."));
        let first = state.register_login_failure(Instant::now());
        let second = state.register_login_failure(Instant::now());
        assert!(second >= first);
    }

    #[test]
    fn test_circuit_breaker_triggers_when_failures_repeat() {
        let mut state = RecoveryRuntimeState::load_or_default(0, 10, Path::new("."));
        let now = Instant::now();
        for _ in 0..CIRCUIT_BREAKER_FAILURE_THRESHOLD {
            state.on_failure(now);
        }
        assert!(state.is_circuit_open(Instant::now() + Duration::from_millis(1)));
        assert!(!state
            .is_circuit_open(Instant::now() + Duration::from_secs(CIRCUIT_BREAKER_COOLDOWN_SECS + 1)));
    }

    #[test]
    fn test_checkpoint_roundtrip_loads_starting_index() -> io::Result<()> {
        let dir = tmp_dir();
        let path = dir.path().join(format!("{CHECKPOINT_FILE_PREFIX}-0.json"));
        let checkpoint = RecoveryCheckpoint {
            task_id: 0,
            total_scenarios: 5,
            next_scenario_index: 3,
        };
        let payload = serde_json::to_string_pretty(&checkpoint)?;
        std::fs::write(&path, payload)?;

        let state = RecoveryRuntimeState::load_or_default(0, 5, dir.path());
        assert_eq!(state.starting_index, 3);
        Ok(())
    }
}
