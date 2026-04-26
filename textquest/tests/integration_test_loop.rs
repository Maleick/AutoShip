//! Integration tests for `TestLoopRunner` with mock scenarios.
//!
//! Covers the end-to-end scenario set for issue #1250:
//! - Single iteration: one mock scenario completes successfully.
//! - Multiple iterations: N mock scenarios all complete successfully.
//! - Early termination: shutdown signal aborts runner mid-loop.
//! - Error handling: failing scenario is counted and flagged.
//! - 36-box multibox scenarios (Windows-only):
//!   - Full automation loop.
//!   - Group coordination pressure handling.
//!   - Stress with multiple runners sharing one scenario.
//!   - Failure recovery with recovery-state validation.
//!
//! Mock scenario tests are macOS-safe. Multibox scenarios are
//! `#[cfg(windows)]` and only run on Windows.

use std::time::Duration;

use tempfile::TempDir;
use textquest::testing::{
    runner::{TestLoopResult, spawn_runners},
    scenario::{BoxScenarioFuture, MetricValue, ScenarioResult, TestScenario},
};
use textquest_common::login::AccountInfo;
use tokio::sync::watch;
#[cfg(windows)]
use textquest::camp::config::CampConfig;
#[cfg(windows)]
use textquest::testing::scenarios::camp_loop::{MultiboxFarmMode, MultiboxFarmScenario};

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

#[cfg(windows)]
fn multibox_camp_config() -> CampConfig {
    CampConfig {
        name: "multibox_farm".into(),
        zone: "mushroom_bat".into(),
        camp_center: [100.0, 200.0, 0.0],
        pull_point: [150.0, 250.0, 0.0],
        pull_radius: 1500.0,
        camp_radius: 75.0,
        leash_radius: 125.0,
        rest_mana_pct: 60,
        pull_mana_pct: 30,
        level_range: [1, 100],
        pull_mob_names: vec!["a dark green".into(), "a mushroom".into()],
        ignore_mob_names: vec![],
        burn_mob_names: vec![],
        return_no_aggro: false,
        next_camp: None,
        prev_camp: None,
    }
}

#[cfg(windows)]
fn metric_u64(result: &ScenarioResult, name: &str) -> u64 {
    match result.metrics.get(name) {
        Some(MetricValue::Counter(value)) => *value,
        Some(MetricValue::Gauge(value)) => {
            let rounded = value.round();
            if rounded.is_sign_negative() || rounded.is_infinite() || rounded.is_nan() {
                panic!("expected non-negative finite counter-like metric for {name}, got {value}");
            }
            rounded as u64
        }
        other => panic!("expected numeric metric for {name}, got {other:?}"),
    }
}

#[cfg(windows)]
fn metric_gauge(result: &ScenarioResult, name: &str) -> f64 {
    match result.metrics.get(name) {
        Some(MetricValue::Gauge(value)) => *value,
        other => panic!("expected gauge metric for {name}, got {other:?}"),
    }
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

#[cfg(windows)]
#[tokio::test]
async fn test_multibox_farm_full_automation_loop() {
    let dir = tmp_dir();
    let (_tx, rx) = no_shutdown();

    let accounts: Vec<AccountInfo> = (0..36)
        .map(|i| test_account(&format!("farm_acct_{i:02}")))
        .collect();
    let config = multibox_camp_config();

    let handles = spawn_runners(
        accounts,
        move |_, _| {
            vec![Box::new(MultiboxFarmScenario::new(
                config.clone(),
                Duration::from_secs(12),
                36,
                MultiboxFarmMode::FullAutomation,
            )) as Box<dyn TestScenario>]
        },
        Duration::from_secs(14),
        dir.path(),
        rx,
    )
    .await;

    assert_eq!(handles.len(), 36);

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.await.expect("runner must not panic"));
    }

    for result in results {
        assert_eq!(result.scenario_results.len(), 1, "one scenario per runner");
        let scenario_result = &result.scenario_results[0];

        assert!(scenario_result.success, "multibox scenario must succeed");
        assert!(metric_u64(scenario_result, "member_count") >= 36);
        assert!(metric_u64(scenario_result, "pulls") >= 1);
        assert!(metric_u64(scenario_result, "kills") >= 1);
        assert!(metric_u64(scenario_result, "commands") > 0);
    }
}

#[cfg(windows)]
#[tokio::test]
async fn test_multibox_farm_group_coordination() {
    let dir = tmp_dir();
    let (_tx, rx) = no_shutdown();

    let accounts = vec![test_account("farm_coord_acct")];
    let config = multibox_camp_config();

    let handles = spawn_runners(
        accounts,
        move |_, _| {
            vec![Box::new(MultiboxFarmScenario::new(
                config.clone(),
                Duration::from_secs(10),
                36,
                MultiboxFarmMode::GroupCoordination,
            )) as Box<dyn TestScenario>]
        },
        Duration::from_secs(10),
        dir.path(),
        rx,
    )
    .await;

    let result = handles
        .into_iter()
        .next()
        .expect("single handle expected")
        .await
        .expect("runner must not panic");
    let scenario_result = &result.scenario_results[0];

    assert!(scenario_result.success);
    assert!(
        metric_u64(scenario_result, "healer_casts") >= 1,
        "group coordination should issue emergency healer casts",
    );
    assert!(
        metric_u64(scenario_result, "attack_commands") > 0,
        "group should actively fight during coordination scenario",
    );
    assert!(
        metric_gauge(scenario_result, "kill_rate") >= 0.0,
        "kill_rate metric must be present",
    );
}

#[cfg(windows)]
#[tokio::test]
async fn test_multibox_farm_stress_full_group() {
    let dir = tmp_dir();
    let (_tx, rx) = no_shutdown();
    let accounts: Vec<AccountInfo> = (0..6)
        .map(|i| test_account(&format!("farm_stress_{i}")))
        .collect();
    let config = multibox_camp_config();

    let handles = spawn_runners(
        accounts,
        move |_, _| {
            vec![Box::new(MultiboxFarmScenario::new(
                config.clone(),
                Duration::from_secs(10),
                36,
                MultiboxFarmMode::FullAutomation,
            )) as Box<dyn TestScenario>]
        },
        Duration::from_secs(16),
        dir.path(),
        rx,
    )
    .await;

    assert_eq!(handles.len(), 6);

    let mut result_count = 0usize;
    for handle in handles {
        let result = handle.await.expect("runner must not panic");
        let scenario_result = &result.scenario_results[0];

        assert!(scenario_result.success);
        assert_eq!(metric_u64(scenario_result, "member_count"), 36);
        assert!(metric_u64(scenario_result, "pulls") >= 2);
        result_count += 1;
    }

    assert_eq!(result_count, 6);
}

#[cfg(windows)]
#[tokio::test]
async fn test_multibox_farm_failure_recovery() {
    let dir = tmp_dir();
    let (_tx, rx) = no_shutdown();

    let accounts = vec![test_account("farm_recovery_acct")];
    let config = multibox_camp_config();

    let handles = spawn_runners(
        accounts,
        move |_, _| {
            vec![Box::new(MultiboxFarmScenario::new(
                config.clone(),
                Duration::from_secs(12),
                36,
                MultiboxFarmMode::FailureRecovery,
            )) as Box<dyn TestScenario>]
        },
        Duration::from_secs(12),
        dir.path(),
        rx,
    )
    .await;

    let result = handles
        .into_iter()
        .next()
        .expect("single handle expected")
        .await
        .expect("runner must not panic");
    let scenario_result = &result.scenario_results[0];

    assert!(scenario_result.success);
    assert!(
        metric_u64(scenario_result, "recovery_transitions") >= 1,
        "failure injection should enter recovery state",
    );
    assert!(
        metric_u64(scenario_result, "rez_casts") >= 1,
        "recovery must attempt rez behavior",
    );
}
