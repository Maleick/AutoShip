//! HTML report generator for TextQuest test sessions.
//!
//! Generates a self-contained HTML5 report (inline CSS/JS) from a session
//! directory, event log, and metrics summary. No external dependencies are
//! required — all chart rendering uses inline SVG.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── TestEvent ────────────────────────────────────────────────────────────────

/// A single timestamped event emitted during a test session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestEvent {
    /// UTC timestamp of the event.
    pub timestamp: DateTime<Utc>,
    /// Account or client that generated this event.
    pub account: String,
    /// Scenario name this event belongs to.
    pub scenario: String,
    /// Kind of event.
    pub kind: TestEventKind,
    /// Optional human-readable message.
    pub message: Option<String>,
}

/// Discriminated event kinds recorded during a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestEventKind {
    /// A mob was killed.
    Kill,
    /// A pull attempt was made.
    Pull,
    /// A scenario started.
    ScenarioStart,
    /// A scenario completed successfully.
    ScenarioSuccess,
    /// A scenario failed.
    ScenarioFailure,
    /// A non-fatal error occurred.
    Error,
    /// Generic informational event.
    Info,
}

impl TestEventKind {
    fn label(&self) -> &'static str {
        match self {
            Self::Kill => "Kill",
            Self::Pull => "Pull",
            Self::ScenarioStart => "Scenario Start",
            Self::ScenarioSuccess => "Scenario Success",
            Self::ScenarioFailure => "Scenario Failure",
            Self::Error => "Error",
            Self::Info => "Info",
        }
    }
}

// ── MetricsSummary ───────────────────────────────────────────────────────────

/// Aggregated metrics for a completed test session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsSummary {
    /// Total number of scenario iterations executed.
    pub total_iterations: u64,
    /// Number of successful iterations.
    pub success_count: u64,
    /// Number of failed iterations.
    pub failure_count: u64,
    /// Total kills across all accounts.
    pub total_kills: u64,
    /// Total pull attempts across all accounts.
    pub total_pulls: u64,
    /// Average DPS across all accounts (0.0 if not recorded).
    pub avg_dps: f64,
    /// Per-scenario metrics: scenario name → key → value.
    pub per_scenario: HashMap<String, HashMap<String, f64>>,
    /// Error counts by error message / type.
    pub error_distribution: HashMap<String, u64>,
    /// Top errors with timestamps (message, timestamp ISO string).
    pub top_errors: Vec<(String, String)>,
    /// Hourly timeline data: pulls per hour and kills per hour keyed by hour index.
    pub timeline: Vec<TimelineBucket>,
}

impl MetricsSummary {
    /// Success rate as a percentage (0–100), or 0 if no iterations ran.
    pub fn success_rate(&self) -> f64 {
        if self.total_iterations == 0 {
            return 0.0;
        }
        (self.success_count as f64 / self.total_iterations as f64) * 100.0
    }

    /// Build a `MetricsSummary` from a slice of events.
    pub fn from_events(events: &[TestEvent]) -> Self {
        let mut summary = Self::default();
        let mut hourly: HashMap<u32, (u64, u64)> = HashMap::new(); // hour → (pulls, kills)

        let mut errors_full: Vec<(String, DateTime<Utc>)> = Vec::new();

        for ev in events {
            let hour = ev.timestamp.format("%H").to_string().parse::<u32>().unwrap_or(0);
            let bucket = hourly.entry(hour).or_insert((0, 0));

            match ev.kind {
                TestEventKind::Kill => {
                    summary.total_kills += 1;
                    bucket.1 += 1;
                }
                TestEventKind::Pull => {
                    summary.total_pulls += 1;
                    bucket.0 += 1;
                }
                TestEventKind::ScenarioSuccess => {
                    summary.total_iterations += 1;
                    summary.success_count += 1;
                }
                TestEventKind::ScenarioFailure => {
                    summary.total_iterations += 1;
                    summary.failure_count += 1;
                }
                TestEventKind::Error => {
                    let msg = ev.message.clone().unwrap_or_else(|| "unknown".to_string());
                    *summary.error_distribution.entry(msg.clone()).or_insert(0) += 1;
                    errors_full.push((msg, ev.timestamp));
                }
                TestEventKind::ScenarioStart | TestEventKind::Info => {}
            }
        }

        // Build timeline sorted by hour.
        let mut hours: Vec<u32> = hourly.keys().copied().collect();
        hours.sort_unstable();
        summary.timeline = hours
            .into_iter()
            .map(|h| {
                let (pulls, kills) = hourly[&h];
                TimelineBucket {
                    hour_label: format!("{h:02}:00"),
                    pulls_per_hour: pulls,
                    kills_per_hour: kills,
                }
            })
            .collect();

        // Top errors — sort by timestamp descending, take 20.
        errors_full.sort_by_key(|e| std::cmp::Reverse(e.1));
        summary.top_errors = errors_full
            .into_iter()
            .take(20)
            .map(|(msg, ts)| (msg, ts.format("%Y-%m-%d %H:%M:%S UTC").to_string()))
            .collect();

        summary
    }
}

/// One time-bucket in the activity timeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimelineBucket {
    /// Human-readable label for this bucket (e.g. "14:00").
    pub hour_label: String,
    /// Pull attempts in this window.
    pub pulls_per_hour: u64,
    /// Kills in this window.
    pub kills_per_hour: u64,
}

// ── ReportGenerator ──────────────────────────────────────────────────────────

/// Generates a self-contained HTML5 report from session data.
pub struct ReportGenerator;

impl ReportGenerator {
    /// Generate an HTML report string.
    ///
    /// # Parameters
    /// - `session_dir`: Path to the session directory (used for session ID display).
    /// - `events`: All events recorded during the session.
    /// - `metrics`: Pre-computed summary; if `None`, built from `events`.
    ///
    /// # Returns
    /// A self-contained HTML5 string with inline CSS and SVG charts.
    pub fn generate(
        session_dir: &Path,
        events: &[TestEvent],
        metrics: Option<&MetricsSummary>,
    ) -> String {
        let computed;
        let m: &MetricsSummary = if let Some(m) = metrics {
            m
        } else {
            computed = MetricsSummary::from_events(events);
            &computed
        };

        let session_id = session_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string());

        let generated_at = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1.0"/>
<title>TextQuest Session Report — {session_id}</title>
{css}
</head>
<body>
{header}
{summary}
{success_bar}
{timeline}
{error_dist}
{scenario_table}
{top_errors}
<footer><small>Generated {generated_at} by TextQuest report engine</small></footer>
</body>
</html>"#,
            session_id = escape_html(&session_id),
            css = Self::css(),
            header = Self::render_header(&session_id, &generated_at),
            summary = Self::render_summary(m),
            success_bar = Self::render_success_bar(m),
            timeline = Self::render_timeline(m),
            error_dist = Self::render_error_distribution(m),
            scenario_table = Self::render_scenario_table(m),
            top_errors = Self::render_top_errors(m),
            generated_at = escape_html(&generated_at),
        );

        html
    }

    // ── CSS ──────────────────────────────────────────────────────────────────

    fn css() -> &'static str {
        r#"<style>
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  background: #0d1117; color: #c9d1d9; line-height: 1.5; padding: 1rem;
}
h1 { color: #58a6ff; font-size: 1.6rem; margin-bottom: 0.25rem; }
h2 { color: #79c0ff; font-size: 1.1rem; margin: 1.5rem 0 0.75rem; border-bottom: 1px solid #30363d; padding-bottom: 0.4rem; }
.container { max-width: 1100px; margin: 0 auto; }
header { margin-bottom: 1.5rem; }
header .meta { color: #8b949e; font-size: 0.85rem; margin-top: 0.2rem; }
.cards { display: flex; flex-wrap: wrap; gap: 1rem; margin-bottom: 1rem; }
.card {
  background: #161b22; border: 1px solid #30363d; border-radius: 8px;
  padding: 1rem 1.25rem; flex: 1 1 140px; min-width: 130px;
}
.card .label { font-size: 0.75rem; color: #8b949e; text-transform: uppercase; letter-spacing: 0.05em; }
.card .value { font-size: 1.75rem; font-weight: 700; color: #58a6ff; margin-top: 0.2rem; }
.card .value.success { color: #3fb950; }
.card .value.failure { color: #f85149; }
.bar-wrap { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 1rem; margin-bottom: 1rem; }
.bar-label { font-size: 0.8rem; color: #8b949e; margin-bottom: 0.4rem; }
.bar-track { background: #f85149; border-radius: 4px; height: 24px; position: relative; overflow: hidden; }
.bar-fill { background: #3fb950; height: 100%; border-radius: 4px 0 0 4px; display: flex; align-items: center; padding-left: 0.5rem; font-size: 0.8rem; font-weight: 600; color: #fff; }
svg.chart { width: 100%; height: 180px; display: block; background: #161b22; border: 1px solid #30363d; border-radius: 8px; margin-bottom: 1rem; overflow: visible; }
table { width: 100%; border-collapse: collapse; background: #161b22; border: 1px solid #30363d; border-radius: 8px; overflow: hidden; margin-bottom: 1rem; }
th { background: #21262d; color: #8b949e; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; padding: 0.6rem 0.75rem; text-align: left; }
td { padding: 0.55rem 0.75rem; border-top: 1px solid #21262d; font-size: 0.875rem; }
tr:hover td { background: #1c2129; }
.err-msg { font-family: ui-monospace, monospace; font-size: 0.8rem; color: #f0883e; }
.ts { color: #8b949e; font-size: 0.78rem; }
footer { text-align: center; color: #484f58; font-size: 0.75rem; margin-top: 2rem; padding-top: 1rem; border-top: 1px solid #21262d; }
@media (max-width: 600px) { .card { flex: 1 1 100%; } }
</style>"#
    }

    // ── Header ───────────────────────────────────────────────────────────────

    fn render_header(session_id: &str, generated_at: &str) -> String {
        format!(
            r#"<div class="container">
<header>
  <h1>TextQuest Session Report</h1>
  <div class="meta">Session: <strong>{session_id}</strong> &nbsp;|&nbsp; Generated: {generated_at}</div>
</header>"#,
            session_id = escape_html(session_id),
            generated_at = escape_html(generated_at),
        )
    }

    // ── Summary cards ────────────────────────────────────────────────────────

    fn render_summary(m: &MetricsSummary) -> String {
        let success_rate = format!("{:.1}%", m.success_rate());
        let dps = format!("{:.1}", m.avg_dps);
        format!(
            r#"<h2>Summary</h2>
<div class="cards">
  <div class="card"><div class="label">Iterations</div><div class="value">{iterations}</div></div>
  <div class="card"><div class="label">Success Rate</div><div class="value success">{success_rate}</div></div>
  <div class="card"><div class="label">Errors</div><div class="value failure">{errors}</div></div>
  <div class="card"><div class="label">Total Kills</div><div class="value">{kills}</div></div>
  <div class="card"><div class="label">Total Pulls</div><div class="value">{pulls}</div></div>
  <div class="card"><div class="label">Avg DPS</div><div class="value">{dps}</div></div>
</div>"#,
            iterations = m.total_iterations,
            success_rate = escape_html(&success_rate),
            errors = m.failure_count,
            kills = m.total_kills,
            pulls = m.total_pulls,
            dps = dps,
        )
    }

    // ── Success/failure bar ──────────────────────────────────────────────────

    fn render_success_bar(m: &MetricsSummary) -> String {
        let rate = m.success_rate().clamp(0.0, 100.0);
        let pct = format!("{rate:.1}%");
        format!(
            r#"<h2>Success / Failure</h2>
<div class="bar-wrap">
  <div class="bar-label">Success rate: {pct} ({success} passed, {fail} failed)</div>
  <div class="bar-track">
    <div class="bar-fill" style="width:{pct};">{pct}</div>
  </div>
</div>"#,
            pct = escape_html(&pct),
            success = m.success_count,
            fail = m.failure_count,
        )
    }

    // ── Timeline chart (SVG bar chart) ───────────────────────────────────────

    fn render_timeline(m: &MetricsSummary) -> String {
        if m.timeline.is_empty() {
            return r#"<h2>Activity Timeline</h2><p style="color:#8b949e;font-size:0.85rem">No timeline data recorded.</p>"#.to_string();
        }

        let max_val = m
            .timeline
            .iter()
            .map(|b| b.kills_per_hour.max(b.pulls_per_hour))
            .max()
            .unwrap_or(1)
            .max(1) as f64;

        let n = m.timeline.len();
        let chart_w = 960.0_f64;
        let chart_h = 140.0_f64;
        let pad_l = 40.0_f64;
        let pad_b = 24.0_f64;
        let bar_area_w = chart_w - pad_l - 10.0;
        let bar_w = (bar_area_w / (n as f64 * 2.5)).max(4.0);
        let gap = bar_area_w / n as f64;

        let mut bars = String::new();
        let mut labels = String::new();

        let color_blue = "#58a6ff";
        let color_green = "#3fb950";
        let color_muted = "#8b949e";

        for (i, bucket) in m.timeline.iter().enumerate() {
            let x_center = pad_l + (i as f64 + 0.5) * gap;

            // Pulls bar (blue)
            let pull_h = (bucket.pulls_per_hour as f64 / max_val) * chart_h;
            let pull_x = x_center - bar_w - 1.0;
            let pull_y = chart_h - pull_h;
            bars.push_str(&format!(
                r#"<rect x="{pull_x:.1}" y="{pull_y:.1}" width="{bar_w:.1}" height="{pull_h:.1}" fill="{color_blue}" opacity="0.8"><title>Pulls: {pulls}</title></rect>"#,
                pull_x = pull_x,
                pull_y = pull_y,
                bar_w = bar_w,
                pull_h = pull_h,
                pulls = bucket.pulls_per_hour,
                color_blue = color_blue,
            ));

            // Kills bar (green)
            let kill_h = (bucket.kills_per_hour as f64 / max_val) * chart_h;
            let kill_x = x_center + 1.0;
            let kill_y = chart_h - kill_h;
            bars.push_str(&format!(
                r#"<rect x="{kill_x:.1}" y="{kill_y:.1}" width="{bar_w:.1}" height="{kill_h:.1}" fill="{color_green}" opacity="0.8"><title>Kills: {kills}</title></rect>"#,
                kill_x = kill_x,
                kill_y = kill_y,
                bar_w = bar_w,
                kill_h = kill_h,
                kills = bucket.kills_per_hour,
                color_green = color_green,
            ));

            // Label every other bucket to avoid crowding.
            if i % 2 == 0 || n <= 6 {
                labels.push_str(&format!(
                    r#"<text x="{x:.1}" y="{y:.1}" text-anchor="middle" fill="{color_muted}" font-size="10">{label}</text>"#,
                    x = x_center,
                    y = chart_h + pad_b - 4.0,
                    label = escape_html(&bucket.hour_label),
                    color_muted = color_muted,
                ));
            }
        }

        format!(
            r#"<h2>Activity Timeline</h2>
<svg class="chart" viewBox="0 0 {chart_w} {total_h}" preserveAspectRatio="none">
  <text x="4" y="10" fill="{color_muted}" font-size="10">pulls/h</text>
  <text x="4" y="22" fill="{color_green}" font-size="9">&#9632;</text>
  <text x="13" y="22" fill="{color_muted}" font-size="9">kills</text>
  <text x="4" y="34" fill="{color_blue}" font-size="9">&#9632;</text>
  <text x="13" y="34" fill="{color_muted}" font-size="9">pulls</text>
  <g transform="translate(0,8)">{bars}{labels}</g>
</svg>"#,
            chart_w = chart_w,
            total_h = chart_h + pad_b + 12.0,
            bars = bars,
            color_blue = color_blue,
            color_green = color_green,
            color_muted = color_muted,
            labels = labels,
        )
    }

    // ── Error distribution (horizontal bar chart) ────────────────────────────

    fn render_error_distribution(m: &MetricsSummary) -> String {
        if m.error_distribution.is_empty() {
            return r#"<h2>Error Distribution</h2><p style="color:#8b949e;font-size:0.85rem">No errors recorded. 🎉</p>"#.to_string();
        }

        let mut entries: Vec<(&String, &u64)> = m.error_distribution.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1));
        entries.truncate(10);

        let max_count = *entries[0].1 as f64;
        let bar_max_w = 300.0_f64;

        let mut rows = String::new();
        for (msg, count) in &entries {
            let w = ((**count as f64) / max_count * bar_max_w) as u64;
            rows.push_str(&format!(
                r#"<tr>
  <td class="err-msg">{msg}</td>
  <td>{count}</td>
  <td><div style="background:#f85149;height:14px;width:{w}px;border-radius:2px;"></div></td>
</tr>"#,
                msg = escape_html(msg),
                count = count,
                w = w,
            ));
        }

        format!(
            r#"<h2>Error Distribution (Top 10)</h2>
<table>
<thead><tr><th>Error</th><th>Count</th><th>Frequency</th></tr></thead>
<tbody>{rows}</tbody>
</table>"#,
            rows = rows,
        )
    }

    // ── Per-scenario metrics table ────────────────────────────────────────────

    fn render_scenario_table(m: &MetricsSummary) -> String {
        if m.per_scenario.is_empty() {
            return r#"<h2>Per-Scenario Metrics</h2><p style="color:#8b949e;font-size:0.85rem">No scenario metrics recorded.</p>"#.to_string();
        }

        // Collect all unique metric keys across scenarios.
        let mut all_keys: Vec<String> = m
            .per_scenario
            .values()
            .flat_map(|kv| kv.keys().cloned())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        all_keys.sort();

        let mut header_cells = String::from("<th>Scenario</th>");
        for k in &all_keys {
            header_cells.push_str(&format!("<th>{}</th>", escape_html(k)));
        }

        let mut scenario_names: Vec<&String> = m.per_scenario.keys().collect();
        scenario_names.sort();

        let mut rows = String::new();
        for name in scenario_names {
            let kv = &m.per_scenario[name];
            let mut row = format!("<td>{}</td>", escape_html(name));
            for k in &all_keys {
                let val = kv.get(k).copied().unwrap_or(0.0);
                row.push_str(&format!("<td>{:.2}</td>", val));
            }
            rows.push_str(&format!("<tr>{row}</tr>", row = row));
        }

        format!(
            r#"<h2>Per-Scenario Metrics</h2>
<table>
<thead><tr>{header}</tr></thead>
<tbody>{rows}</tbody>
</table>"#,
            header = header_cells,
            rows = rows,
        )
    }

    // ── Top errors list ───────────────────────────────────────────────────────

    fn render_top_errors(m: &MetricsSummary) -> String {
        if m.top_errors.is_empty() {
            return r#"<h2>Top Errors</h2><p style="color:#8b949e;font-size:0.85rem">No errors logged.</p>
</div>"#.to_string();
        }

        let mut rows = String::new();
        for (msg, ts) in &m.top_errors {
            rows.push_str(&format!(
                r#"<tr><td class="ts">{ts}</td><td class="err-msg">{msg}</td></tr>"#,
                ts = escape_html(ts),
                msg = escape_html(msg),
            ));
        }

        format!(
            r#"<h2>Top Errors (most recent first)</h2>
<table>
<thead><tr><th>Timestamp</th><th>Message</th></tr></thead>
<tbody>{rows}</tbody>
</table>
</div>"#,
            rows = rows,
        )
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Escape HTML special characters.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_events() -> Vec<TestEvent> {
        let base = DateTime::parse_from_rfc3339("2026-04-22T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        vec![
            TestEvent {
                timestamp: base,
                account: "Acct01".into(),
                scenario: "combat".into(),
                kind: TestEventKind::Pull,
                message: None,
            },
            TestEvent {
                timestamp: base,
                account: "Acct01".into(),
                scenario: "combat".into(),
                kind: TestEventKind::Kill,
                message: None,
            },
            TestEvent {
                timestamp: base,
                account: "Acct01".into(),
                scenario: "combat".into(),
                kind: TestEventKind::ScenarioSuccess,
                message: None,
            },
            TestEvent {
                timestamp: base,
                account: "Acct02".into(),
                scenario: "combat".into(),
                kind: TestEventKind::ScenarioFailure,
                message: None,
            },
            TestEvent {
                timestamp: base,
                account: "Acct02".into(),
                scenario: "combat".into(),
                kind: TestEventKind::Error,
                message: Some("Connection timeout".into()),
            },
        ]
    }

    #[test]
    fn test_generates_valid_html() {
        let dir = PathBuf::from("/tmp/session-2026-04-22-100000-abcdef");
        let events = sample_events();
        let html = ReportGenerator::generate(&dir, &events, None);

        assert!(html.contains("<!DOCTYPE html>"), "must start with doctype");
        assert!(html.contains("</html>"), "must end with </html>");
        assert!(html.contains("<title>"), "must have title element");
        assert!(html.contains("TextQuest Session Report"), "must have report heading");
    }

    #[test]
    fn test_summary_cards_populated() {
        let dir = PathBuf::from("/tmp/test-session");
        let events = sample_events();
        let html = ReportGenerator::generate(&dir, &events, None);

        // 1 kill, 1 pull, 1 success, 1 failure, 1 error
        assert!(html.contains("Total Kills") || html.contains("Kills"), "kills card present");
        assert!(html.contains("Total Pulls") || html.contains("Pulls"), "pulls card present");
    }

    #[test]
    fn test_empty_events_generates_html() {
        let dir = PathBuf::from("/tmp/empty-session");
        let html = ReportGenerator::generate(&dir, &[], None);

        assert!(html.contains("<!DOCTYPE html>"), "empty events: doctype present");
        assert!(html.contains("</html>"), "empty events: close tag present");
        assert!(html.contains("0"), "empty events: zeros shown");
    }

    #[test]
    fn test_large_event_count() {
        let base = DateTime::parse_from_rfc3339("2026-04-22T14:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let events: Vec<TestEvent> = (0..1200)
            .map(|i| TestEvent {
                timestamp: base,
                account: format!("Acct{:02}", i % 36),
                scenario: "stress".into(),
                kind: if i % 5 == 0 {
                    TestEventKind::Error
                } else if i % 2 == 0 {
                    TestEventKind::Kill
                } else {
                    TestEventKind::Pull
                },
                message: if i % 5 == 0 {
                    Some(format!("Error kind {}", i % 10))
                } else {
                    None
                },
            })
            .collect();

        let dir = PathBuf::from("/tmp/large-session");
        let html = ReportGenerator::generate(&dir, &events, None);
        assert!(html.contains("<!DOCTYPE html>"), "large: doctype present");
        // Top errors capped at 20 — shouldn't have > 20 <tr> entries in that section.
        let top_err_section = html
            .find("Top Errors")
            .map(|pos| &html[pos..])
            .unwrap_or("");
        let tr_count = top_err_section.matches("<tr>").count();
        // header row + up to 20 data rows
        assert!(tr_count <= 21, "top errors capped: got {tr_count}");
    }

    #[test]
    fn test_html_escaping() {
        let base = DateTime::parse_from_rfc3339("2026-04-22T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let events = vec![TestEvent {
            timestamp: base,
            account: "Acct<script>".into(),
            scenario: "xss & test".into(),
            kind: TestEventKind::Error,
            message: Some("<script>alert(1)</script>".into()),
        }];
        let dir = PathBuf::from("/tmp/xss-session");
        let html = ReportGenerator::generate(&dir, &events, None);

        assert!(
            !html.contains("<script>alert(1)</script>"),
            "raw script tags must not appear in output"
        );
        assert!(html.contains("&lt;script&gt;"), "angle brackets must be escaped");
    }

    #[test]
    fn test_success_rate_calculation() {
        let mut m = MetricsSummary::default();
        assert_eq!(m.success_rate(), 0.0, "zero iterations → 0%");

        m.total_iterations = 4;
        m.success_count = 3;
        m.failure_count = 1;
        let rate = m.success_rate();
        assert!((rate - 75.0).abs() < 0.01, "3/4 = 75%, got {rate}");
    }

    #[test]
    fn test_metrics_from_events_counts() {
        let events = sample_events();
        let m = MetricsSummary::from_events(&events);

        assert_eq!(m.total_kills, 1);
        assert_eq!(m.total_pulls, 1);
        assert_eq!(m.success_count, 1);
        assert_eq!(m.failure_count, 1);
        assert_eq!(m.error_distribution.get("Connection timeout"), Some(&1));
    }

    #[test]
    fn test_pre_computed_metrics_used() {
        let dir = PathBuf::from("/tmp/precomputed-session");
        let mut m = MetricsSummary::default();
        m.total_iterations = 99;
        m.success_count = 99;
        m.total_kills = 500;
        let html = ReportGenerator::generate(&dir, &[], Some(&m));

        assert!(html.contains("99"), "pre-computed iterations shown");
        assert!(html.contains("500"), "pre-computed kills shown");
    }

    #[test]
    fn test_html5_doctype() {
        let dir = PathBuf::from("/tmp/doctype-session");
        let html = ReportGenerator::generate(&dir, &[], None);
        assert!(html.trim_start().starts_with("<!DOCTYPE html>"), "HTML5 doctype required");
    }

    #[test]
    fn test_no_external_resources() {
        let dir = PathBuf::from("/tmp/self-contained-session");
        let html = ReportGenerator::generate(&dir, &[], None);

        // Must not reference external URLs in src/href attributes.
        assert!(!html.contains("src=\"http"), "no external src URLs");
        assert!(!html.contains("href=\"http"), "no external href URLs");
        assert!(!html.contains("<link"), "no external stylesheets via <link>");
        assert!(!html.contains("<script src"), "no external script tags");
    }
}
