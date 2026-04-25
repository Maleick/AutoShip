//! Patch-day binary diff reconciliation panel for the Debug screen.

use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Wrap},
};
use serde_json::Value;

use crate::tui::{
    app::{ActivePanel, App},
    theme::Theme,
    ui::widgets::{panel, themed_header_row},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PatchReconciliationFilter {
    #[default]
    All,
    Matched,
    Unmatched,
}

impl PatchReconciliationFilter {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Matched => "matched",
            Self::Unmatched => "unmatched",
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Matched,
            Self::Matched => Self::Unmatched,
            Self::Unmatched => Self::All,
        }
    }

    fn allows(self, entry: &PatchReconciliationEntry) -> bool {
        match self {
            Self::All => true,
            Self::Matched => entry.matched,
            Self::Unmatched => !entry.matched,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PatchReconciliationEntry {
    pub name: String,
    pub old_address: Option<String>,
    pub new_address: Option<String>,
    pub confidence: Option<f64>,
    pub matched: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PatchReconciliationReport {
    pub old_patch: String,
    pub new_patch: String,
    pub matched_count: usize,
    pub unmatched_count: usize,
    pub entries: Vec<PatchReconciliationEntry>,
}

#[derive(Debug, Clone)]
pub struct PatchReconciliationState {
    source_path: PathBuf,
    loaded_modified: Option<SystemTime>,
    pub filter: PatchReconciliationFilter,
    pub report: Option<PatchReconciliationReport>,
    pub last_error: Option<String>,
}

impl Default for PatchReconciliationState {
    fn default() -> Self {
        Self::new()
    }
}

impl PatchReconciliationState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            source_path: default_patch_report_path(),
            loaded_modified: None,
            filter: PatchReconciliationFilter::All,
            report: None,
            last_error: None,
        }
    }

    #[must_use]
    pub fn from_report(report: PatchReconciliationReport) -> Self {
        Self {
            report: Some(report),
            ..Self::new()
        }
    }

    #[must_use]
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    #[must_use]
    pub fn cycle_filter(&mut self) -> PatchReconciliationFilter {
        self.filter = self.filter.next();
        self.filter
    }

    pub fn load_if_changed(&mut self) {
        let metadata = match fs::metadata(&self.source_path) {
            Ok(metadata) => metadata,
            Err(e) => {
                if self.report.is_none() {
                    self.last_error =
                        Some(format!("Waiting for {} ({e})", self.source_path.display()));
                }
                return;
            }
        };

        let modified = metadata.modified().ok();
        if self.report.is_some() && modified.is_some() && self.loaded_modified == modified {
            return;
        }

        if let Err(e) = self.reload() {
            self.last_error = Some(e);
        }
    }

    pub fn reload(&mut self) -> Result<(), String> {
        let text = fs::read_to_string(&self.source_path)
            .map_err(|e| format!("Failed to read {}: {e}", self.source_path.display()))?;
        let report = parse_patch_report(&text)?;
        self.loaded_modified = fs::metadata(&self.source_path)
            .ok()
            .and_then(|metadata| metadata.modified().ok());
        self.report = Some(report);
        self.last_error = None;
        Ok(())
    }
}

pub fn draw_patch_reconciliation_panel(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &mut App,
) {
    let theme = app.theme.clone();
    let is_active = app.active_panel == ActivePanel::DebugPatchReconciliation;
    let state = &mut app.patch_reconciliation_state;
    state.load_if_changed();
    draw_patch_reconciliation_state(frame, area, state, is_active, &theme);
}

fn draw_patch_reconciliation_state(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    state: &PatchReconciliationState,
    is_active: bool,
    t: &Theme,
) {
    let border_style = if is_active {
        t.border_active
    } else {
        t.border_dim
    };
    let title = format!(" Patch Reconciliation  filter:{} ", state.filter.label());
    let block = panel(title, border_style, t);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let Some(report) = &state.report else {
        let msg = state
            .last_error
            .as_deref()
            .unwrap_or("Waiting for patch-report.json");
        frame.render_widget(
            Paragraph::new(msg)
                .style(Style::default().fg(t.text_muted))
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let summary = format!(
        "Last diff: {} -> {}   Matched: {}  Unmatched: {}   f filter",
        report.old_patch, report.new_patch, report.matched_count, report.unmatched_count
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            summary,
            Style::default().fg(t.text_secondary),
        ))),
        chunks[0],
    );

    let rows = report
        .entries
        .iter()
        .filter(|entry| state.filter.allows(entry))
        .map(|entry| {
            let status_style = if entry.matched {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            };
            let row_style = if entry.matched {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            };
            let new_address = entry
                .new_address
                .as_deref()
                .unwrap_or("NOT FOUND")
                .to_string();
            let detail = entry.confidence.map_or_else(
                || {
                    entry
                        .notes
                        .clone()
                        .unwrap_or_else(|| "needs RE".to_string())
                },
                |confidence| format!("conf {confidence:.2}"),
            );

            Row::new(vec![
                Cell::from(Span::styled(
                    if entry.matched {
                        "matched"
                    } else {
                        "unmatched"
                    },
                    status_style,
                )),
                Cell::from(entry.name.clone()).style(row_style),
                Cell::from(
                    entry
                        .old_address
                        .clone()
                        .unwrap_or_else(|| "unknown".into()),
                )
                .style(row_style),
                Cell::from(new_address).style(row_style),
                Cell::from(detail).style(row_style),
            ])
        })
        .collect::<Vec<_>>();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Min(14),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(10),
        ],
    )
    .header(themed_header_row(
        &["Status", "Function", "Old", "New", "Detail"],
        t,
    ));
    frame.render_widget(table, chunks[1]);
}

pub fn parse_patch_report(text: &str) -> Result<PatchReconciliationReport, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("invalid patch-report.json: {e}"))?;
    let entries_value = value
        .get("entries")
        .or_else(|| value.get("functions"))
        .or_else(|| value.get("results"))
        .and_then(Value::as_array)
        .ok_or_else(|| "patch-report.json missing entries array".to_string())?;

    let entries = entries_value
        .iter()
        .map(parse_entry)
        .collect::<Result<Vec<_>, _>>()?;
    let computed_matched = entries.iter().filter(|entry| entry.matched).count();
    let computed_unmatched = entries.len().saturating_sub(computed_matched);

    Ok(PatchReconciliationReport {
        old_patch: string_field(&value, &["old_patch", "old", "from", "source_patch"])
            .unwrap_or_else(|| "unknown".to_string()),
        new_patch: string_field(&value, &["new_patch", "new", "to", "target_patch"])
            .unwrap_or_else(|| "unknown".to_string()),
        matched_count: usize_field(&value, &["matched", "matched_count"])
            .unwrap_or(computed_matched),
        unmatched_count: usize_field(&value, &["unmatched", "unmatched_count"])
            .unwrap_or(computed_unmatched),
        entries,
    })
}

fn parse_entry(value: &Value) -> Result<PatchReconciliationEntry, String> {
    let name = string_field(value, &["name", "function", "symbol"])
        .ok_or_else(|| "patch-report entry missing name".to_string())?;
    let old_address = address_field(value, &["old_address", "old_preferred", "old", "from"]);
    let new_address = address_field(value, &["new_address", "new_preferred", "new", "to"]);
    let confidence = number_field(value, &["confidence", "conf"]);
    let status = string_field(value, &["status", "result"]).map(|status| status.to_lowercase());
    let matched = value
        .get("matched")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            new_address.is_some()
                && status.as_deref() != Some("unmatched")
                && status.as_deref() != Some("missing")
        });

    Ok(PatchReconciliationEntry {
        name,
        old_address,
        new_address,
        confidence,
        matched,
        notes: string_field(value, &["notes", "note", "action"]),
    })
}

fn string_field(value: &Value, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        value.get(*name).and_then(|field| match field {
            Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
    })
}

fn usize_field(value: &Value, names: &[&str]) -> Option<usize> {
    names.iter().find_map(|name| {
        value
            .get(*name)
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
    })
}

fn number_field(value: &Value, names: &[&str]) -> Option<f64> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_f64))
}

fn address_field(value: &Value, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        value.get(*name).and_then(|field| match field {
            Value::Number(n) => n.as_u64().map(|addr| format!("0x{addr:X}")),
            Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("not found") {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            _ => None,
        })
    })
}

fn default_patch_report_path() -> PathBuf {
    std::env::var_os("TEXTQUEST_PATCH_REPORT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("patch-report.json"))
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    const PATCH_REPORT_FIXTURE: &str = r#"{
  "old_patch": "20260310",
  "new_patch": "20260401",
  "matched": 2,
  "unmatched": 1,
  "entries": [
    {
      "name": "CastSpell",
      "old_address": "0x140D9F20",
      "new_address": "0x140DA120",
      "confidence": 0.97,
      "matched": true
    },
    {
      "name": "DoAttack",
      "old_address": "0x1431B890",
      "new_address": "0x1431C110",
      "confidence": 0.94,
      "matched": true
    },
    {
      "name": "memcheck4",
      "old_address": "0x14029912",
      "new_address": null,
      "matched": false,
      "notes": "needs RE"
    }
  ]
}"#;

    #[test]
    fn fixture_patch_report_renders_summary_and_rows() {
        let report = parse_patch_report(PATCH_REPORT_FIXTURE).expect("fixture parses");
        let state = PatchReconciliationState::from_report(report);
        let rendered = render_state(&state, 96, 10);

        assert!(rendered.contains("Patch Reconciliation"));
        assert!(rendered.contains("20260310 -> 20260401"));
        assert!(rendered.contains("Matched: 2"));
        assert!(rendered.contains("Unmatched: 1"));
        assert!(rendered.contains("CastSpell"));
        assert!(rendered.contains("0x140DA120"));
        assert!(rendered.contains("memcheck4"));
        assert!(rendered.contains("NOT FOUND"));
    }

    #[test]
    fn unmatched_filter_hides_matched_rows() {
        let report = parse_patch_report(PATCH_REPORT_FIXTURE).expect("fixture parses");
        let mut state = PatchReconciliationState::from_report(report);
        state.filter = PatchReconciliationFilter::Unmatched;
        let rendered = render_state(&state, 96, 10);

        assert!(rendered.contains("memcheck4"));
        assert!(!rendered.contains("CastSpell"));
        assert!(!rendered.contains("DoAttack"));
    }

    fn render_state(state: &PatchReconciliationState, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let theme = crate::tui::theme::dark_modern();
        terminal
            .draw(|frame| {
                draw_patch_reconciliation_state(frame, frame.area(), state, true, &theme);
            })
            .expect("draw");
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
