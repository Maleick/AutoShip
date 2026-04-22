//! Multi-runner spawning for concurrent per-account test execution.
//!
//! `TestLoopRunner` drives a [`TestScenario`] for a bounded duration on behalf
//! of one account.  `spawn_runners` launches one runner per account as a
//! separate tokio task and returns the [`JoinHandle`]s for lifecycle management.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use tokio::{
    sync::watch,
    task::JoinHandle,
};

use super::scenario::{ScenarioResult, TestScenario};
use textquest_common::login::AccountInfo;

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
        Self {
            account,
            task_id,
            scenarios,
            duration,
            output_dir: output_dir.as_ref().to_owned(),
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

        for scenario in self.scenarios.iter_mut() {
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

            let result = scenario.run(remaining).await;
            tracing::debug!(
                task_id = self.task_id,
                scenario = scenario.name(),
                success = result.success,
                "scenario complete"
            );
            results.push(result);
        }

        TestLoopResult::from_results(self.account, self.task_id, results)
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

        let runner = TestLoopRunner::new(
            account,
            task_id,
            scenarios,
            duration,
            dir.as_ref(),
            rx,
        );

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
}
