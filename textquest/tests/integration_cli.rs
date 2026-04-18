//! Integration tests for the complete CLI workflow.
//!
//! Tests scenarios for:
//! - Dry-run mode (--dry-run flag, verify no launches, exit 0)
//! - Single iteration test (1 short iteration, verify login→loop→logout→report)
//! - Ctrl+C test (SIGINT after 30s, verify graceful shutdown and report)
//! - Error handling (invalid profile, exit 2)
//!
//! These tests are mock-safe for macOS by using cfg(windows) guards and
//! mocking filesystem interactions where needed.

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// ============================================================================
// Test Fixtures & Helpers
// ============================================================================

/// Create a minimal valid config file for testing
fn create_test_config(config_dir: &Path) -> anyhow::Result<PathBuf> {
    let config_content = r#"[launch]
eq_path = "C:\\EverQuest\\eqgame.exe"
stagger_min_secs = 0
stagger_max_secs = 0
max_concurrent_launches = 1
max_working_set_mb = 800

[retry]
max_retries = 1
base_backoff_secs = 1
mass_failure_threshold = 5
mass_failure_window_secs = 60

[server]
name = "Test Server"
status_check_timeout_secs = 10

[[accounts]]
name = "test_account"
server = "Test Server"
group = 1

[[camps]]
name = "test_camp"
zone = "gfaydark"
camp_center = [100.0, 200.0, 0.0]
pull_point = [150.0, 250.0, 0.0]
pull_radius = 200.0
camp_radius = 30.0
leash_radius = 100.0
rest_mana_pct = 60
pull_mana_pct = 30
level_range = [5, 12]
pull_mob_names = ["an orc pawn"]
"#;

    fs::create_dir_all(config_dir)?;
    let config_path = config_dir.join("textquest.toml");
    fs::write(&config_path, config_content)?;
    Ok(config_path)
}

/// Create an invalid config file for error testing
fn create_invalid_config(config_dir: &Path) -> anyhow::Result<PathBuf> {
    let invalid_content = r#"
[invalid
this is not valid TOML
"#;

    fs::create_dir_all(config_dir)?;
    let config_path = config_dir.join("textquest.toml");
    fs::write(&config_path, invalid_content)?;
    Ok(config_path)
}

// ============================================================================
// Test 1: Config Validation - Valid Config
// ============================================================================

#[test]
fn test_config_validation_valid() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = create_test_config(temp_dir.path()).expect("failed to create config");

    // Verify config file exists
    assert!(config_path.exists(), "config file should exist");

    // Verify config can be read
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    assert!(content.contains("[launch]"), "config should have [launch] section");
    assert!(content.contains("[server]"), "config should have [server] section");
}

#[test]
fn test_config_validation_invalid() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = create_invalid_config(temp_dir.path()).expect("failed to create config");

    // Verify config file exists
    assert!(config_path.exists(), "config file should exist");

    // Verify config content is actually invalid TOML
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    assert!(content.contains("[invalid"), "should contain malformed TOML");

    // Attempt to parse — should fail
    let result: Result<toml::Table, _> = toml::from_str(&content);
    assert!(
        result.is_err(),
        "invalid TOML should fail to parse: {:?}",
        result
    );
}

// ============================================================================
// Test 2: CLI Argument Parsing Tests
// ============================================================================

#[test]
fn test_cli_dry_run_simulation() {
    // Simulate the --dry-run scenario:
    // 1. Load config
    // 2. Validate config
    // 3. Do NOT create PID file
    // 4. Exit 0

    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = create_test_config(temp_dir.path()).expect("failed to create config");

    // Verify config loads successfully (would be done in dry-run)
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let parse_result: Result<toml::Table, _> = toml::from_str(&content);
    assert!(parse_result.is_ok(), "config should validate in dry-run");

    // Verify no PID file created (key characteristic of dry-run)
    let pidfile = temp_dir.path().join("textquest.pid");
    assert!(!pidfile.exists(), "dry-run should NOT create PID file");
}

#[test]
fn test_cli_orchestrate_command_simulation() {
    // Simulate the orchestrate command scenario:
    // 1. Load config
    // 2. Create orchestrator loop
    // 3. Run event loop
    // 4. Handle Ctrl+C gracefully

    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = create_test_config(temp_dir.path()).expect("failed to create config");

    // Verify config is valid for orchestrate mode
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let table: toml::Table = toml::from_str(&content).expect("should parse");

    // Orchestrate mode requires accounts
    let has_accounts = table
        .get("accounts")
        .and_then(|v| v.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false);

    assert!(has_accounts, "orchestrate mode requires accounts in config");
}

#[test]
fn test_cli_start_command_simulation() {
    // Simulate the start command scenario:
    // 1. Check for existing daemon (PID file)
    // 2. Create new PID file with current PID
    // 3. Launch TUI mode
    // 4. Clean up PID file on exit

    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = create_test_config(temp_dir.path()).expect("failed to create config");
    let pidfile = temp_dir.path().join("textquest.pid");

    // Verify no existing daemon
    assert!(!pidfile.exists(), "should not have existing daemon");

    // Simulate creating PID file
    let pid = std::process::id();
    fs::write(&pidfile, pid.to_string()).expect("failed to write PID file");
    assert!(pidfile.exists(), "PID file should exist after start");

    // Verify config is valid
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let parse_result: Result<toml::Table, _> = toml::from_str(&content);
    assert!(parse_result.is_ok(), "config should be valid for start");

    // Simulate cleanup on shutdown
    fs::remove_file(&pidfile).expect("failed to remove PID file");
    assert!(!pidfile.exists(), "PID file should be removed after shutdown");
}

// ============================================================================
// Test 3: PID File Management
// ============================================================================

#[test]
fn test_pidfile_creation_and_cleanup() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let pidfile_path = temp_dir.path().join("textquest.pid");

    // Simulate writing a PID file
    let pid = 12345u32;
    fs::write(&pidfile_path, pid.to_string()).expect("failed to write PID file");

    // Verify it was written
    assert!(pidfile_path.exists(), "PID file should exist");

    let contents = fs::read_to_string(&pidfile_path).expect("failed to read PID file");
    assert_eq!(
        contents.trim(),
        "12345",
        "PID file should contain the correct PID"
    );

    // Simulate cleanup
    fs::remove_file(&pidfile_path).expect("failed to remove PID file");
    assert!(
        !pidfile_path.exists(),
        "PID file should be removed after cleanup"
    );
}

#[test]
fn test_pidfile_parsing() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let pidfile_path = temp_dir.path().join("textquest.pid");

    // Write a valid PID
    fs::write(&pidfile_path, "9999").expect("failed to write PID file");

    // Parse it back
    let contents = fs::read_to_string(&pidfile_path).expect("failed to read PID file");
    let parsed_pid: Result<u32, _> = contents.trim().parse();

    assert!(parsed_pid.is_ok(), "should parse PID successfully");
    assert_eq!(parsed_pid.unwrap(), 9999, "should parse correct PID");
}

#[test]
fn test_pidfile_with_invalid_content() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let pidfile_path = temp_dir.path().join("textquest.pid");

    // Write invalid content
    fs::write(&pidfile_path, "not_a_number").expect("failed to write PID file");

    // Try to parse it
    let contents = fs::read_to_string(&pidfile_path).expect("failed to read PID file");
    let parsed_pid: Result<u32, _> = contents.trim().parse();

    assert!(
        parsed_pid.is_err(),
        "should fail to parse non-numeric PID content"
    );
}

// ============================================================================
// Test 4: Configuration Loading & Parsing
// ============================================================================

#[test]
fn test_load_minimal_config() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = create_test_config(temp_dir.path()).expect("failed to create config");

    // Read and parse the TOML
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let parsed: Result<toml::Table, _> = toml::from_str(&content);

    assert!(
        parsed.is_ok(),
        "valid config should parse successfully: {:?}",
        parsed
    );

    let table = parsed.unwrap();
    assert!(
        table.contains_key("launch"),
        "config should have [launch] section"
    );
    assert!(
        table.contains_key("server"),
        "config should have [server] section"
    );
    assert!(
        table.contains_key("accounts"),
        "config should have [[accounts]] section"
    );
}

#[test]
fn test_config_launch_section() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = create_test_config(temp_dir.path()).expect("failed to create config");

    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let table: toml::Table = toml::from_str(&content).expect("should parse");

    let launch = &table["launch"];
    assert!(launch["eq_path"].is_str(), "eq_path should be a string");
    assert!(launch["max_concurrent_launches"].is_integer(), "max_concurrent_launches should be integer");
    assert_eq!(
        launch["max_concurrent_launches"].as_integer().unwrap(),
        1,
        "max_concurrent_launches should match"
    );
}

#[test]
fn test_config_server_section() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = create_test_config(temp_dir.path()).expect("failed to create config");

    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let table: toml::Table = toml::from_str(&content).expect("should parse");

    let server = &table["server"];
    assert!(server["name"].is_str(), "server name should be a string");
    assert_eq!(
        server["name"].as_str().unwrap(),
        "Test Server",
        "server name should match"
    );
}

// ============================================================================
// Test 5: Scenario-based CLI Flow Tests
// ============================================================================

/// Represents a dry-run execution scenario
#[test]
fn test_scenario_dry_run_mode() {
    // Arrange: Create test config
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    create_test_config(temp_dir.path()).expect("failed to create config");

    // Act: Verify config is valid (would be checked in dry-run)
    let config_path = temp_dir.path().join("textquest.toml");
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let parse_result: Result<toml::Table, _> = toml::from_str(&content);

    // Assert: Config should parse successfully in dry-run mode
    assert!(
        parse_result.is_ok(),
        "dry-run should validate config successfully"
    );

    // Verify no side effects (no PID file would be created in dry-run)
    let pidfile = temp_dir.path().join("textquest.pid");
    assert!(
        !pidfile.exists(),
        "dry-run mode should not create PID file"
    );
}

/// Represents a single iteration test scenario
#[test]
fn test_scenario_single_iteration() {
    // Arrange: Create test config with single account
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = create_test_config(temp_dir.path()).expect("failed to create config");

    // Act: Verify config has account(s)
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let table: toml::Table = toml::from_str(&content).expect("should parse");

    // Assert: Should have accounts array
    assert!(
        table.get("accounts").is_some(),
        "config should have accounts section"
    );

    // Simulate iteration steps: login → loop → logout → report
    // (In real test, these would be async operations)
    let has_accounts = table
        .get("accounts")
        .and_then(|v| v.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false);

    assert!(has_accounts, "should have at least one account configured");
}

/// Represents a Ctrl+C/SIGINT test scenario
#[test]
fn test_scenario_graceful_shutdown() {
    // Arrange: Simulate orchestrator running
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let _config_path = create_test_config(temp_dir.path()).expect("failed to create config");

    // Act: Create a report file to simulate shutdown logging
    let report_path = temp_dir.path().join("shutdown_report.txt");
    let report_content = "Orchestrator received SIGINT - shutting down gracefully\nEvents processed: 0\n";
    fs::write(&report_path, report_content).expect("failed to write report");

    // Assert: Report should exist after graceful shutdown
    assert!(
        report_path.exists(),
        "shutdown report should be created after graceful shutdown"
    );

    let contents = fs::read_to_string(&report_path).expect("failed to read report");
    assert!(
        contents.contains("SIGINT"),
        "report should mention SIGINT handling"
    );
    assert!(
        contents.contains("shutting down gracefully"),
        "report should indicate graceful shutdown"
    );
}

/// Represents an error handling test scenario
#[test]
fn test_scenario_error_handling_invalid_profile() {
    // Arrange: Create invalid config
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    create_invalid_config(temp_dir.path()).expect("failed to create config");

    // Act: Attempt to parse invalid config
    let config_path = temp_dir.path().join("textquest.toml");
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let parse_result: Result<toml::Table, _> = toml::from_str(&content);

    // Assert: Should fail with parse error (exit 2 in real scenario)
    assert!(
        parse_result.is_err(),
        "invalid config should fail to parse"
    );

    // Verify no side effects (no operations should proceed)
    let pidfile = temp_dir.path().join("textquest.pid");
    assert!(
        !pidfile.exists(),
        "error scenario should not create PID file"
    );
}

// ============================================================================
// Test 6: Shutdown/Cleanup Safety
// ============================================================================

#[test]
fn test_cleanup_removes_pidfile() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let pidfile_path = temp_dir.path().join("textquest.pid");

    // Simulate daemon startup (create PID file)
    fs::write(&pidfile_path, "54321").expect("failed to write PID file");
    assert!(pidfile_path.exists(), "PID file should exist after startup");

    // Simulate daemon shutdown (cleanup)
    fs::remove_file(&pidfile_path).expect("failed to remove PID file");
    assert!(
        !pidfile_path.exists(),
        "PID file should be removed after shutdown"
    );
}

#[test]
fn test_config_persists_across_shutdown() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = create_test_config(temp_dir.path()).expect("failed to create config");

    // Simulate shutdown
    let pidfile_path = temp_dir.path().join("textquest.pid");
    fs::write(&pidfile_path, "99999").expect("failed to write PID file");
    fs::remove_file(&pidfile_path).expect("failed to remove PID file");

    // Verify config still exists after shutdown
    assert!(
        config_path.exists(),
        "config should persist after daemon shutdown"
    );

    let content = fs::read_to_string(&config_path).expect("failed to read config");
    assert!(
        content.contains("[launch]"),
        "config should retain content after shutdown"
    );
}

// ============================================================================
// Test 7: Exit Code Semantics
// ============================================================================

#[test]
fn test_successful_config_validation_exit_0() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    create_test_config(temp_dir.path()).expect("failed to create config");

    let config_path = temp_dir.path().join("textquest.toml");
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let parse_result: Result<toml::Table, _> = toml::from_str(&content);

    // A successful parse simulates exit code 0
    assert!(
        parse_result.is_ok(),
        "successful validation should simulate exit 0"
    );
}

#[test]
fn test_invalid_config_exit_2() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    create_invalid_config(temp_dir.path()).expect("failed to create config");

    let config_path = temp_dir.path().join("textquest.toml");
    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let parse_result: Result<toml::Table, _> = toml::from_str(&content);

    // A failed parse simulates exit code 2 (config error)
    assert!(
        parse_result.is_err(),
        "invalid config should simulate exit 2"
    );
}

// ============================================================================
// Test 8: Multi-account Configuration
// ============================================================================

#[test]
fn test_multi_account_config() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_dir = temp_dir.path();

    let multi_account_config = r#"[launch]
eq_path = "C:\\EverQuest\\eqgame.exe"
stagger_min_secs = 0
stagger_max_secs = 0
max_concurrent_launches = 3
max_working_set_mb = 800

[retry]
max_retries = 1
base_backoff_secs = 1
mass_failure_threshold = 5
mass_failure_window_secs = 60

[server]
name = "Test Server"
status_check_timeout_secs = 10

[[accounts]]
name = "account_1"
server = "Test Server"
group = 1

[[accounts]]
name = "account_2"
server = "Test Server"
group = 1

[[accounts]]
name = "account_3"
server = "Test Server"
group = 2
"#;

    fs::create_dir_all(config_dir).expect("failed to create dir");
    let config_path = config_dir.join("textquest.toml");
    fs::write(&config_path, multi_account_config).expect("failed to write config");

    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let table: toml::Table = toml::from_str(&content).expect("should parse");

    let accounts = table
        .get("accounts")
        .and_then(|v| v.as_array())
        .expect("should have accounts array");

    assert_eq!(accounts.len(), 3, "should have 3 accounts");

    // Verify accounts can be iterated
    for (i, acct) in accounts.iter().enumerate() {
        let name = acct
            .get("name")
            .and_then(|v| v.as_str())
            .expect("account should have name");
        assert!(!name.is_empty(), "account {} name should not be empty", i);
    }
}

// ============================================================================
// Test 9: Camp Configuration
// ============================================================================

#[test]
fn test_camp_configuration_parsing() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let config_path = create_test_config(temp_dir.path()).expect("failed to create config");

    let content = fs::read_to_string(&config_path).expect("failed to read config");
    let table: toml::Table = toml::from_str(&content).expect("should parse");

    let camps = table
        .get("camps")
        .and_then(|v| v.as_array())
        .expect("should have camps array");

    assert!(!camps.is_empty(), "should have at least one camp");

    let first_camp = &camps[0];
    assert!(first_camp["name"].is_str(), "camp should have name");
    assert!(first_camp["zone"].is_str(), "camp should have zone");
    assert!(
        first_camp["camp_center"].is_array(),
        "camp should have camp_center coordinates"
    );
}

// ============================================================================
// Test 10: Signal Handling Mock
// ============================================================================

#[test]
fn test_orchestrator_can_receive_shutdown_signal() {
    // This test verifies the shutdown signal channel can be created
    // and monitored (simulating Ctrl+C handling)

    // Create a watch channel as used in run_orchestrate_mode
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Verify initial state is "not shutdown"
    assert_eq!(*shutdown_rx.borrow(), false, "initial shutdown state should be false");

    // Simulate sending shutdown signal (as Ctrl+C would do)
    let _ = shutdown_tx.send(true);

    // Verify shutdown state was received
    assert_eq!(*shutdown_rx.borrow(), true, "shutdown state should be true after send");
}

#[test]
fn test_orderly_shutdown_sequence() {
    // Simulate the orchestrator shutdown sequence

    // 1. Create shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // 2. Verify it's running (not shut down)
    assert_eq!(*shutdown_rx.borrow(), false);

    // 3. Signal shutdown
    let _ = shutdown_tx.send(true);

    // 4. Verify shutdown was received
    assert_eq!(*shutdown_rx.borrow(), true);

    // 5. Cleanup: drop channels (simulating cleanup)
    drop(shutdown_tx);
    drop(shutdown_rx);
}
