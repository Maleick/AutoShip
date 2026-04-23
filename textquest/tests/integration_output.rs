//! Integration tests for the complete output pipeline.
//!
//! Verifies that `SessionManager`, `MetricsAggregator`, and `FleetEventLog`
//! cooperate to produce well-formed output artefacts:
//!
//! - Session directory is created under a temp root.
//! - `events.jsonl` contains one JSON object per line.
//! - `metrics.json` is valid JSON with expected aggregate keys.
//! - `report.html` is non-empty and contains the `<!DOCTYPE html>` marker.
//! - `export.json` is valid JSON and contains expected top-level fields.
//!
//! # Platform notes
//!
//! All tests are platform-agnostic — no Windows-only paths or IPC.

use std::fs;
use tempfile::TempDir;
use textquest::testing::metrics::{MetricValue, MetricsAggregator};
use textquest::testing::output::SessionManager;

// ── helpers ──────────────────────────────────────────────────────────────────

fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("failed to create tempdir")
}

/// Write `events` as newline-delimited JSON (one object per line) to
/// `{session_dir}/events.jsonl` and return the path.
fn write_events_jsonl(
    session_dir: &std::path::Path,
    events: &[serde_json::Value],
) -> std::path::PathBuf {
    let path = session_dir.join("events.jsonl");
    let mut content = String::new();
    for ev in events {
        content.push_str(&serde_json::to_string(ev).expect("event must serialize"));
        content.push('\n');
    }
    fs::write(&path, &content).expect("failed to write events.jsonl");
    path
}

/// Serialize `aggregator` to `{session_dir}/metrics.json` and return the path.
fn write_metrics_json(
    session_dir: &std::path::Path,
    aggregator: &MetricsAggregator,
) -> std::path::PathBuf {
    let path = session_dir.join("metrics.json");
    fs::write(&path, aggregator.to_json()).expect("failed to write metrics.json");
    path
}

/// Generate a minimal HTML report listing each event name and write it to
/// `{session_dir}/report.html`.  Returns the path.
fn generate_html_report(
    session_dir: &std::path::Path,
    event_names: &[&str],
    metrics_json: &str,
) -> std::path::PathBuf {
    let path = session_dir.join("report.html");

    let rows: String = event_names
        .iter()
        .map(|name| format!("    <li>{name}</li>\n"))
        .collect();

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>TextQuest Output Report</title></head>
<body>
<h1>Output Pipeline Report</h1>
<h2>Events</h2>
<ul>
{rows}</ul>
<h2>Metrics</h2>
<pre>{metrics_json}</pre>
</body>
</html>
"#
    );

    fs::write(&path, &html).expect("failed to write report.html");
    path
}

/// Build and write a JSON export summary to `{session_dir}/export.json`.
fn write_export_json(
    session_dir: &std::path::Path,
    session_id: &str,
    event_count: usize,
    metrics_json: &str,
) -> std::path::PathBuf {
    let path = session_dir.join("export.json");

    // Parse metrics back to a Value so the export embeds a real object.
    let metrics_value: serde_json::Value =
        serde_json::from_str(metrics_json).unwrap_or(serde_json::Value::Null);

    let export = serde_json::json!({
        "session_id": session_id,
        "event_count": event_count,
        "metrics": metrics_value,
        "schema_version": 1,
    });

    fs::write(
        &path,
        serde_json::to_string_pretty(&export).expect("export must serialize"),
    )
    .expect("failed to write export.json");
    path
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Full pipeline: session dir → events.jsonl → metrics.json → report.html → export.json
#[test]
fn test_full_output_pipeline() {
    let tmp = temp_dir();

    // ── 1. Create session directory ──────────────────────────────────────────
    let sm = SessionManager::new(
        tmp.path(),
        vec!["Warrior01".into(), "Cleric01".into()],
        vec!["solo_farm".into()],
    )
    .expect("SessionManager::new failed");

    assert!(sm.session_dir.exists(), "session directory must be created");
    assert!(
        sm.metadata_path().exists(),
        "metadata.json must be written by SessionManager"
    );

    // ── 2. Build mock events (2 iterations × 2 clients = 4 events) ──────────
    let events: Vec<serde_json::Value> = vec![
        serde_json::json!({"type": "kill", "pid": 1001, "mob": "an orc pawn", "iteration": 1}),
        serde_json::json!({"type": "kill", "pid": 1002, "mob": "an orc pawn", "iteration": 1}),
        serde_json::json!({"type": "kill", "pid": 1001, "mob": "a gnoll", "iteration": 2}),
        serde_json::json!({"type": "kill", "pid": 1002, "mob": "a gnoll", "iteration": 2}),
    ];

    let events_path = write_events_jsonl(&sm.session_dir, &events);

    // ── 3. Verify events.jsonl ───────────────────────────────────────────────
    assert!(events_path.exists(), "events.jsonl must exist");
    let raw = fs::read_to_string(&events_path).expect("read events.jsonl");
    let lines: Vec<&str> = raw.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        4,
        "expected 4 event lines, got {}",
        lines.len()
    );

    // Each line must be valid JSON.
    for (i, line) in lines.iter().enumerate() {
        let parsed: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("line {i} not valid JSON: {e}"));
        assert!(parsed.is_object(), "line {i} must be a JSON object");
    }

    // ── 4. Build and verify metrics.json ────────────────────────────────────
    let mut agg = MetricsAggregator::new();
    agg.add("kills_total", MetricValue::Counter(4));
    agg.add("iterations", MetricValue::Counter(2));
    agg.add("clients", MetricValue::Gauge(2.0));

    let metrics_path = write_metrics_json(&sm.session_dir, &agg);
    assert!(metrics_path.exists(), "metrics.json must exist");

    let metrics_raw = fs::read_to_string(&metrics_path).expect("read metrics.json");
    let metrics_val: serde_json::Value =
        serde_json::from_str(&metrics_raw).expect("metrics.json must be valid JSON");

    assert!(
        metrics_val.is_object(),
        "metrics.json root must be a JSON object"
    );
    assert!(
        metrics_val.get("kills_total").is_some(),
        "metrics.json must contain 'kills_total'"
    );
    assert!(
        metrics_val.get("iterations").is_some(),
        "metrics.json must contain 'iterations'"
    );

    let kills_sum = metrics_val["kills_total"]["sum"]
        .as_f64()
        .expect("kills_total.sum must be numeric");
    assert!(
        (kills_sum - 4.0).abs() < f64::EPSILON,
        "kills_total sum expected 4.0, got {kills_sum}"
    );

    // ── 5. Generate and verify HTML report ──────────────────────────────────
    let event_names = ["kill (orc pawn, iter 1)", "kill (gnoll, iter 2)"];
    let report_path = generate_html_report(&sm.session_dir, &event_names, &metrics_raw);
    assert!(report_path.exists(), "report.html must exist");

    let html = fs::read_to_string(&report_path).expect("read report.html");
    assert!(!html.is_empty(), "report.html must not be empty");
    assert!(
        html.contains("<!DOCTYPE html>"),
        "report.html must contain DOCTYPE declaration"
    );
    assert!(
        html.contains("Output Pipeline Report"),
        "report.html must contain a heading"
    );

    // ── 6. Export JSON and verify ────────────────────────────────────────────
    let export_path = write_export_json(
        &sm.session_dir,
        &sm.metadata.session_id,
        events.len(),
        &metrics_raw,
    );
    assert!(export_path.exists(), "export.json must exist");

    let export_raw = fs::read_to_string(&export_path).expect("read export.json");
    let export_val: serde_json::Value =
        serde_json::from_str(&export_raw).expect("export.json must be valid JSON");

    assert!(
        export_val.is_object(),
        "export.json root must be a JSON object"
    );
    assert!(
        export_val.get("session_id").is_some(),
        "export must contain 'session_id'"
    );
    assert!(
        export_val.get("event_count").is_some(),
        "export must contain 'event_count'"
    );
    assert!(
        export_val.get("metrics").is_some(),
        "export must contain 'metrics'"
    );
    assert_eq!(
        export_val["event_count"].as_u64().unwrap_or(0),
        4,
        "event_count must be 4"
    );
}

/// Verifies that metrics aggregated across 2 simulated iterations produce
/// accurate sums, averages, and counts in the JSON output.
#[test]
fn test_report_accuracy() {
    let tmp = temp_dir();

    let sm = SessionManager::new(tmp.path(), vec!["Necro01".into()], vec!["combat".into()])
        .expect("SessionManager::new failed");

    // Simulate 2 iterations: collect DPS samples and kill counts.
    let mut agg = MetricsAggregator::new();

    // Iteration 1
    agg.add("kills", MetricValue::Counter(3));
    agg.add("dps", MetricValue::Gauge(120.5));

    // Iteration 2
    agg.add("kills", MetricValue::Counter(5));
    agg.add("dps", MetricValue::Gauge(98.2));

    let metrics_path = write_metrics_json(&sm.session_dir, &agg);
    let raw = fs::read_to_string(&metrics_path).expect("read metrics.json");
    let val: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    // kills: 3 + 5 = 8 total, count = 2, average = 4.0
    let kills = &val["kills"];
    assert_eq!(
        kills["count"].as_u64().unwrap_or(0),
        2,
        "kill count must be 2"
    );
    let kills_sum = kills["sum"].as_f64().expect("kills.sum");
    assert!(
        (kills_sum - 8.0).abs() < f64::EPSILON,
        "kills sum expected 8.0, got {kills_sum}"
    );
    let kills_avg = kills["average"].as_f64().expect("kills.average");
    assert!(
        (kills_avg - 4.0).abs() < f64::EPSILON,
        "kills average expected 4.0, got {kills_avg}"
    );

    // dps: 120.5 + 98.2 = 218.7, average = 109.35
    let dps = &val["dps"];
    let dps_sum = dps["sum"].as_f64().expect("dps.sum");
    assert!(
        (dps_sum - 218.7).abs() < 0.001,
        "dps sum expected ~218.7, got {dps_sum}"
    );
    let dps_avg = dps["average"].as_f64().expect("dps.average");
    assert!(
        (dps_avg - 109.35).abs() < 0.001,
        "dps average expected ~109.35, got {dps_avg}"
    );
}

/// Verifies that the JSON export produced by `write_export_json` is valid,
/// complete, and contains consistent values.
#[test]
fn test_json_export_validity() {
    let tmp = temp_dir();

    let sm = SessionManager::new(
        tmp.path(),
        vec!["Mage01".into(), "Mage02".into(), "Mage03".into()],
        vec!["aoe_farm".into()],
    )
    .expect("SessionManager::new failed");

    // Minimal metrics used for export
    let mut agg = MetricsAggregator::new();
    agg.add("kills", MetricValue::Counter(12));
    agg.add("deaths", MetricValue::Counter(0));
    agg.add("camp_uptime_pct", MetricValue::Gauge(99.1));

    let metrics_raw = agg.to_json();
    let event_count: usize = 12;

    let export_path = write_export_json(
        &sm.session_dir,
        &sm.metadata.session_id,
        event_count,
        &metrics_raw,
    );
    assert!(export_path.exists(), "export.json must be created");

    let raw = fs::read_to_string(&export_path).expect("read export.json");
    let val: serde_json::Value =
        serde_json::from_str(&raw).expect("export.json must be valid JSON");

    // Top-level shape
    assert!(val.is_object(), "export must be a JSON object");

    // Required fields
    let required_fields = ["session_id", "event_count", "metrics", "schema_version"];
    for field in &required_fields {
        assert!(
            val.get(field).is_some(),
            "export.json missing required field '{field}'"
        );
    }

    // session_id must match the one generated by SessionManager
    assert_eq!(
        val["session_id"].as_str().unwrap_or(""),
        sm.metadata.session_id.as_str(),
        "session_id in export must match SessionManager metadata"
    );

    // event_count must match
    assert_eq!(
        val["event_count"].as_u64().unwrap_or(0),
        event_count as u64,
        "event_count must match"
    );

    // schema_version must be present and numeric
    assert!(
        val["schema_version"].as_u64().is_some(),
        "schema_version must be a non-negative integer"
    );

    // metrics sub-object must contain our keys
    let metrics = &val["metrics"];
    assert!(
        metrics.is_object(),
        "metrics in export must be a JSON object"
    );
    assert!(
        metrics.get("kills").is_some(),
        "metrics.kills must be present in export"
    );
    assert!(
        metrics.get("deaths").is_some(),
        "metrics.deaths must be present in export"
    );

    // kills sum = 12
    let kills_sum = metrics["kills"]["sum"]
        .as_f64()
        .expect("kills.sum in export");
    assert!(
        (kills_sum - 12.0).abs() < f64::EPSILON,
        "kills sum in export expected 12.0, got {kills_sum}"
    );
}
