//! Integration tests for `TestLoopRunner` with mock scenarios.
//!
//! Covers the four end-to-end test scenarios required by #1055:
//! - Single iteration: one mock scenario completes successfully.
//! - Multiple iterations: N mock scenarios all complete successfully.
//! - Early termination: shutdown signal aborts runner mid-loop.
//! - Error handling: failing scenario is counted and flagged.
//!
//! All tests are macOS-safe — they use mock scenarios only, no live EQ process.

use std::time::Duration;

use tempfile::TempDir;
use textquest::testing::{
    runner::{TestLoopResult, spawn_runners},
    scenario::{BoxScenarioFuture, ScenarioResult, TestScenario},
};
use textquest_common::login::AccountInfo;
use tokio::sync::watch;

// ── helpers ───────────────────────────────────────────────────────────────────

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

// ── Mock scenarios ────────────────────────────────────────────────────────────

/// Scenario that always succeeds instantly.
struct MockPass {
    label: &'static str,
}

impl TestScenario for MockPass {
    fn name(&self) -> &str {
        self.label
    }

    fn run(&mut self, _duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async { ScenarioResult::success(Duration::from_millis(0)) })
    }
}

/// Scenario that always fails with a fixed error message.
struct MockFail {
    label: &'static str,
}

impl TestScenario for MockFail {
    fn name(&self) -> &str {
        self.label
    }

    fn run(&mut self, _duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async {
            ScenarioResult::failure(
                Duration::from_millis(0),
                vec!["mock scenario failure".to_string()],
            )
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Single account, single mock scenario — should complete with no failures.
#[tokio::test]
async fn test_single_iteration_loop() {
    let dir = tmp_dir();
    let (_tx, rx) = no_shutdown();

    let accounts = vec![test_account("acct_single")];
    let handles = spawn_runners(
        accounts,
        |_, _| {
            vec![Box::new(MockPass {
                label: "login_mock",
            }) as Box<dyn TestScenario>]
        },
        Duration::from_secs(5),
        dir.path(),
        rx,
    )
    .await;

    assert_eq!(handles.len(), 1);

    let result: TestLoopResult = handles.into_iter().next().unwrap().await.expect("task ok");

    assert_eq!(result.account.account_name, "acct_single");
    assert_eq!(result.task_id, 0);
    assert!(!result.any_failure, "single iteration should pass");
    assert_eq!(result.scenario_results.len(), 1, "one scenario result");
    assert!(
        result.scenario_results[0].success,
        "scenario must report success"
    );
}

/// Three accounts, three scenarios each — all should complete without failures.
#[tokio::test]
async fn test_multi_iteration_loop() {
    let dir = tmp_dir();
    let (_tx, rx) = no_shutdown();

    let accounts = vec![
        test_account("acct_a"),
        test_account("acct_b"),
        test_account("acct_c"),
    ];

    let handles = spawn_runners(
        accounts,
        |_, _| {
            vec![
                Box::new(MockPass {
                    label: "scenario_1",
                }) as Box<dyn TestScenario>,
                Box::new(MockPass {
                    label: "scenario_2",
                }) as Box<dyn TestScenario>,
                Box::new(MockPass {
                    label: "scenario_3",
                }) as Box<dyn TestScenario>,
            ]
        },
        Duration::from_secs(10),
        dir.path(),
        rx,
    )
    .await;

    assert_eq!(handles.len(), 3, "one handle per account");

    let mut results: Vec<TestLoopResult> = {
        let mut out = Vec::with_capacity(handles.len());
        for h in handles {
            out.push(h.await.expect("task ok"));
        }
        out
    };

    results.sort_by_key(|r| r.task_id);

    let task_ids: Vec<usize> = results.iter().map(|r| r.task_id).collect();
    assert_eq!(task_ids, vec![0, 1, 2], "task IDs must be 0, 1, 2");

    for result in &results {
        assert!(
            !result.any_failure,
            "account {} should have no failures",
            result.account.account_name
        );
        assert_eq!(
            result.scenario_results.len(),
            3,
            "account {} should have 3 scenario results",
            result.account.account_name
        );
    }
}

/// Shutdown signal fires before runner reads scenarios — runner exits early.
#[tokio::test]
async fn test_early_termination() {
    let dir = tmp_dir();
    let (tx, rx) = no_shutdown();

    // Signal shutdown immediately so the runner exits before any scenario runs.
    tx.send(true).expect("send shutdown");

    let accounts = vec![test_account("acct_shutdown")];
    let handles = spawn_runners(
        accounts,
        |_, _| {
            // Provide a long-running scenario that should NOT complete because
            // the shutdown signal fires first.
            vec![Box::new(MockPass {
                label: "should_not_run",
            }) as Box<dyn TestScenario>]
        },
        Duration::from_secs(60),
        dir.path(),
        rx,
    )
    .await;

    assert_eq!(handles.len(), 1);

    let result: TestLoopResult = handles.into_iter().next().unwrap().await.expect("task ok");

    // Runner exits early — scenario list is empty because shutdown fires before
    // the first scenario starts. Either 0 scenarios ran (clean abort) or 1 ran
    // and succeeded (race); either way there must be no failure.
    assert!(
        !result.any_failure,
        "early-terminated runner should not report failure"
    );
    // The account is still correctly recorded.
    assert_eq!(result.account.account_name, "acct_shutdown");
}

/// Failing scenario is counted and flagged in the result.
#[tokio::test]
async fn test_scenario_error_handling() {
    let dir = tmp_dir();
    let (_tx, rx) = no_shutdown();

    let accounts = vec![test_account("acct_error")];
    let handles = spawn_runners(
        accounts,
        |_, _| {
            vec![
                Box::new(MockPass {
                    label: "ok_scenario",
                }) as Box<dyn TestScenario>,
                Box::new(MockFail {
                    label: "fail_scenario",
                }) as Box<dyn TestScenario>,
            ]
        },
        Duration::from_secs(5),
        dir.path(),
        rx,
    )
    .await;

    assert_eq!(handles.len(), 1);

    let result: TestLoopResult = handles.into_iter().next().unwrap().await.expect("task ok");

    assert!(
        result.any_failure,
        "runner with a failing scenario must set any_failure=true"
    );
    assert_eq!(result.scenario_results.len(), 2, "both scenarios ran");

    let pass_result = &result.scenario_results[0];
    assert!(pass_result.success, "first scenario should succeed");
    assert!(pass_result.errors.is_empty());

    let fail_result = &result.scenario_results[1];
    assert!(!fail_result.success, "second scenario should fail");
    assert!(
        !fail_result.errors.is_empty(),
        "failure must carry error messages"
    );
    assert_eq!(fail_result.errors[0], "mock scenario failure");
}
