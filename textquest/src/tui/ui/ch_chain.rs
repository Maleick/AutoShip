//! CH (Complete Heal) Chain configuration and monitoring panel.
//!
//! Provides a dedicated UI for managing the cleric heal chain,
//! including chain ordering, timing, target selection, and real-time status.

use crate::tui::{
    cast::CastDisplay,
    ui::widgets::{render_cast_bar, truncate_inline},
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

const COMPACT_LAYOUT_WIDTH_THRESHOLD: u16 = 72;
const COMPACT_LAYOUT_HEIGHT_THRESHOLD: u16 = 20;
const HEADER_TARGET_DECORATION_WIDTH: u16 = 22;
const HEADER_TARGET_MIN_WIDTH: usize = 8;

/// A single cleric in the CH chain.
#[derive(Debug, Clone)]
pub struct ChainCleric {
    /// Character name.
    pub name: String,
    /// Process ID.
    pub pid: u32,
    /// Position in the chain (1-indexed).
    pub position: u8,
    /// Individual timing offset (ms) for lag compensation.
    pub timing_offset_ms: i32,
    /// Optional exact/provisional cast strip for this cleric.
    pub cast_display: Option<CastDisplay>,
    /// Current casting state.
    pub cast_state: CastState,
}

/// Cast state for a chain member.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CastState {
    /// Idle, waiting for turn.
    Idle,
    /// Currently casting (progress 0.0 - 1.0).
    Casting(f32),
    /// Just finished casting.
    Completed,
    /// Missed their cast window.
    Missed,
}

/// CH chain configuration state for the TUI panel.
pub struct ChChainPanelState {
    /// Whether the CH panel is visible.
    pub active: bool,
    /// Clerics in the chain.
    pub clerics: Vec<ChainCleric>,
    /// Currently selected cleric index.
    pub selected: usize,
    /// Chain target name.
    pub target_name: String,
    /// Chain target spawn ID.
    pub target_id: u32,
    /// Base cast time in seconds.
    pub cast_time_secs: f32,
    /// Overlap buffer in seconds.
    pub overlap_buffer_secs: f32,
    /// Inter-cleric delay in seconds.
    pub chain_delay_secs: f32,
    /// Whether adaptive timing is enabled.
    pub adaptive: bool,
    /// Current chain health stats.
    pub stats: ChainStats,
    /// Saved presets.
    pub presets: Vec<ChainPreset>,
    /// Selected preset index.
    pub selected_preset: usize,
    /// Which sub-panel is focused.
    pub focus: ChPanelFocus,
}

/// Focus areas within the CH chain panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChPanelFocus {
    ChainOrder,
    Timing,
    Target,
    Presets,
}

/// Chain health statistics.
#[derive(Debug, Clone, Default)]
pub struct ChainStats {
    pub total_heals: u32,
    pub missed_heals: u32,
    pub late_casts: u32,
    pub avg_cast_time_ms: f32,
    pub chain_uptime_pct: f32,
}

/// A saved chain configuration preset.
#[derive(Debug, Clone)]
pub struct ChainPreset {
    pub name: String,
    pub zone_hint: String,
    pub cleric_pids: Vec<u32>,
    pub interval_secs: f32,
    pub adaptive: bool,
}

impl ChChainPanelState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            clerics: Vec::new(),
            selected: 0,
            target_name: String::new(),
            target_id: 0,
            cast_time_secs: 10.0,
            overlap_buffer_secs: 0.5,
            chain_delay_secs: 2.5,
            adaptive: false,
            stats: ChainStats::default(),
            presets: Vec::new(),
            selected_preset: 0,
            focus: ChPanelFocus::ChainOrder,
        }
    }

    /// Move the selected cleric up in the chain.
    pub fn move_up(&mut self) {
        if self.selected > 0 && self.selected < self.clerics.len() {
            self.clerics.swap(self.selected, self.selected - 1);
            // Update positions
            self.clerics[self.selected].position = (self.selected + 1) as u8;
            self.clerics[self.selected - 1].position = self.selected as u8;
            self.selected -= 1;
        }
    }

    /// Move the selected cleric down in the chain.
    pub fn move_down(&mut self) {
        if self.selected + 1 < self.clerics.len() {
            self.clerics.swap(self.selected, self.selected + 1);
            self.clerics[self.selected].position = (self.selected + 1) as u8;
            self.clerics[self.selected + 1].position = (self.selected + 2) as u8;
            self.selected += 1;
        }
    }

    /// Select next cleric.
    pub fn select_next(&mut self) {
        if !self.clerics.is_empty() {
            self.selected = (self.selected + 1) % self.clerics.len();
        }
    }

    /// Select previous cleric.
    pub fn select_prev(&mut self) {
        if !self.clerics.is_empty() {
            if self.selected == 0 {
                self.selected = self.clerics.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    /// Cycle focus between sub-panels.
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            ChPanelFocus::ChainOrder => ChPanelFocus::Timing,
            ChPanelFocus::Timing => ChPanelFocus::Target,
            ChPanelFocus::Target => ChPanelFocus::Presets,
            ChPanelFocus::Presets => ChPanelFocus::ChainOrder,
        };
    }

    /// Get chain health as a simple score (0.0 = broken, 1.0 = perfect).
    pub fn chain_health(&self) -> f32 {
        if self.stats.total_heals == 0 {
            return 1.0;
        }
        let success_rate = 1.0 - (self.stats.missed_heals as f32 / self.stats.total_heals as f32);
        success_rate.clamp(0.0, 1.0)
    }
}

/// Widget that draws the CH chain configuration panel.
pub struct ChChainWidget<'a> {
    state: &'a ChChainPanelState,
    accent_color: Color,
}

impl<'a> ChChainWidget<'a> {
    pub fn new(state: &'a ChChainPanelState) -> Self {
        Self {
            state,
            accent_color: Color::Cyan,
        }
    }

    pub fn accent_color(mut self, color: Color) -> Self {
        self.accent_color = color;
        self
    }
}

impl Widget for ChChainWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" CH Chain · panel ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.accent_color)); // magenta border

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 5 || inner.width < 20 {
            return;
        }

        let compact = inner.width < COMPACT_LAYOUT_WIDTH_THRESHOLD
            || inner.height < COMPACT_LAYOUT_HEIGHT_THRESHOLD;
        let layout = if compact {
            Layout::vertical([
                Constraint::Length(2), // Target header
                Constraint::Min(2),    // Chain member list
                Constraint::Length(2), // Compact footer
            ])
            .split(inner)
        } else {
            Layout::vertical([
                Constraint::Length(2), // Target header
                Constraint::Min(4),    // Chain member list
                Constraint::Length(2), // Timing config
                Constraint::Length(2), // Stats
            ])
            .split(inner)
        };

        self.render_header(layout[0], buf, compact);
        self.render_chain_list(layout[1], buf, compact);
        if compact {
            self.render_compact_footer(layout[2], buf);
        } else {
            self.render_timing(layout[2], buf);
            self.render_stats(layout[3], buf);
        }
    }
}

impl ChChainWidget<'_> {
    fn render_header(&self, area: Rect, buf: &mut Buffer, compact: bool) {
        let target_prefix = if compact { "Tgt:" } else { "Target:" };
        let target_name = if self.state.target_name.is_empty() {
            String::from("(none)")
        } else {
            self.state.target_name.clone()
        };
        let target_budget = area
            .width
            .saturating_sub(HEADER_TARGET_DECORATION_WIDTH)
            .max(HEADER_TARGET_MIN_WIDTH as u16) as usize;
        let target_label = truncate_inline(&target_name, target_budget);

        let adaptive_label = if self.state.adaptive {
            Span::styled(
                if compact { " ADAPT " } else { " ADAPTIVE " },
                Style::default().fg(Color::Green),
            )
        } else {
            Span::styled(" FIXED ", Style::default().fg(Color::Yellow))
        };

        let lines = vec![
            Line::from(vec![
                Span::styled(
                    format!("{target_prefix} {target_label} [{}]", self.state.target_id),
                    Style::default().fg(Color::White),
                ),
                Span::raw("  "),
                adaptive_label,
            ]),
            Line::from(Span::styled(
                if compact {
                    format!(
                        "Chain {} clr  {:.1}s gap",
                        self.state.clerics.len(),
                        self.state.chain_delay_secs
                    )
                } else {
                    format!(
                        "Chain: {} clerics, {:.1}s delay",
                        self.state.clerics.len(),
                        self.state.chain_delay_secs
                    )
                },
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let para = Paragraph::new(lines);
        para.render(area, buf);
    }

    fn render_chain_list(&self, area: Rect, buf: &mut Buffer, compact: bool) {
        let block = Block::default()
            .title(if compact { " Chain " } else { " Chain Order " })
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        block.render(area, buf);

        if self.state.clerics.is_empty() {
            let hint = Paragraph::new(Span::styled(
                "No clerics in chain. Use :ch add <pid> to add.",
                Style::default().fg(Color::DarkGray),
            ));
            hint.render(inner, buf);
            return;
        }

        let visible_clerics = self.state.clerics.len().min(inner.height as usize);
        let extra_capacity = inner.height as usize - visible_clerics;
        let tight_height = extra_capacity
            < self
                .state
                .clerics
                .iter()
                .filter(|cleric| cleric.cast_display.is_some())
                .count();
        let mut y = inner.y;
        let mut remaining_details = extra_capacity;
        for (i, cleric) in self.state.clerics.iter().enumerate() {
            if y >= inner.y + inner.height {
                break;
            }
            let is_selected = i == self.state.selected;
            let base_style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(self.accent_color)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            // Cast state indicator
            let (state_char, state_color) = match cleric.cast_state {
                CastState::Idle => (if compact { "." } else { "○" }, Color::DarkGray),
                CastState::Casting(pct) => {
                    if pct > 0.75 {
                        (if compact { "*" } else { "◕" }, Color::Green)
                    } else if pct > 0.25 {
                        (if compact { ">" } else { "◑" }, Color::Yellow)
                    } else {
                        (if compact { "-" } else { "◔" }, Color::White)
                    }
                }
                CastState::Completed => (if compact { "*" } else { "●" }, Color::Green),
                CastState::Missed => (if compact { "x" } else { "✗" }, Color::Red),
            };
            let pos_text = format!(" {}. ", cleric.position);
            let offset_text = if cleric.timing_offset_ms != 0 {
                format!(" {:+}ms", cleric.timing_offset_ms)
            } else {
                String::new()
            };
            let has_cast_details = cleric.cast_display.is_some();
            let has_capacity_for_details = remaining_details > 0;
            let force_show_details =
                is_selected || matches!(cleric.cast_state, CastState::Casting(_));
            let layout_allows_details = !tight_height || force_show_details;
            let eligible_detail =
                has_cast_details && has_capacity_for_details && layout_allows_details;
            let inline_cast = cleric
                .cast_display
                .as_ref()
                .filter(|_| compact || (tight_height && !eligible_detail))
                .map(|cast| inline_cast_summary(cast, inner.width.saturating_sub(20) as usize))
                .filter(|summary| !summary.is_empty())
                .map(|summary| format!("  {summary}"))
                .unwrap_or_default();
            let name_budget = inner.width.saturating_sub(
                (pos_text.chars().count()
                    + 2
                    + state_char.chars().count()
                    + offset_text.chars().count()) as u16,
            ) as usize;
            let name_budget = name_budget
                .saturating_sub(inline_cast.chars().count())
                .max(4);
            let name_label = truncate_inline(&cleric.name, name_budget);
            let row = Line::from(vec![
                Span::styled(pos_text, base_style),
                Span::styled(name_label, base_style),
                Span::styled("  ", base_style),
                Span::styled(
                    state_char,
                    Style::default().fg(state_color).bg(if is_selected {
                        self.accent_color
                    } else {
                        Color::Reset
                    }),
                ),
                Span::styled(
                    offset_text,
                    Style::default().fg(Color::DarkGray).bg(if is_selected {
                        self.accent_color
                    } else {
                        Color::Reset
                    }),
                ),
                Span::styled(
                    inline_cast,
                    Style::default().fg(Color::DarkGray).bg(if is_selected {
                        self.accent_color
                    } else {
                        Color::Reset
                    }),
                ),
            ]);
            buf.set_line(inner.x, y, &row, inner.width);
            y += 1;

            if eligible_detail
                && let Some(cast_display) = &cleric.cast_display
                && y < inner.y + inner.height
            {
                let mut cast_line = render_cast_bar(
                    cast_display,
                    inner.width.saturating_sub(2) as usize,
                    if cast_display.exact {
                        Color::Green
                    } else {
                        Color::White
                    },
                    if cast_display.exact {
                        Color::Green
                    } else {
                        Color::Yellow
                    },
                    if cast_display.exact {
                        Color::White
                    } else {
                        Color::Gray
                    },
                    Color::DarkGray,
                );
                cast_line.spans.insert(0, Span::raw("  "));
                if is_selected {
                    cast_line = apply_row_background(cast_line, self.accent_color);
                }
                buf.set_line(inner.x, y, &cast_line, inner.width);
                y += 1;
                remaining_details = remaining_details.saturating_sub(1);
            }
        }
    }

    fn render_timing(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Timing ")
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        block.render(area, buf);

        let line = Line::from(vec![
            Span::styled("Cast: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.1}s", self.state.cast_time_secs),
                Style::default().fg(Color::White),
            ),
            Span::raw("  "),
            Span::styled("Overlap: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.1}s", self.state.overlap_buffer_secs),
                Style::default().fg(Color::White),
            ),
            Span::raw("  "),
            Span::styled("Delay: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.1}s", self.state.chain_delay_secs),
                Style::default().fg(Color::White),
            ),
        ]);

        let para = Paragraph::new(line);
        para.render(inner, buf);
    }

    fn render_stats(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Chain Health ")
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 {
            return;
        }

        let health = self.state.chain_health();
        let health_color = if health > 0.9 {
            Color::Green
        } else if health > 0.7 {
            Color::Yellow
        } else {
            Color::Red
        };

        let stats = &self.state.stats;
        let line = Line::from(vec![
            Span::styled("Health: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.0}%", health * 100.0),
                Style::default().fg(health_color),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "Heals: {}  Missed: {}  Late: {}",
                    stats.total_heals, stats.missed_heals, stats.late_casts
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        let para = Paragraph::new(line);
        para.render(inner, buf);
    }

    fn render_compact_footer(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Timing / Health ")
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 {
            return;
        }

        let health_pct = self.state.chain_health() * 100.0;
        let summary = format!(
            "Cast {:.1}s  Gap {:.1}s  HP {:.0}%  Miss {}",
            self.state.cast_time_secs,
            self.state.chain_delay_secs,
            health_pct,
            self.state.stats.missed_heals
        );
        let line = Line::from(Span::styled(
            truncate_inline(&summary, inner.width as usize),
            Style::default().fg(Color::DarkGray),
        ));
        Paragraph::new(line).render(inner, buf);
    }
}

fn inline_cast_summary(cast: &CastDisplay, budget: usize) -> String {
    let label = cast.preferred_label(true);
    let suffix = cast
        .remaining_secs
        .map(|remaining| format!(" {remaining:.1}s"))
        .or_else(|| cast.status_text.as_ref().map(|status| format!(" {status}")))
        .unwrap_or_default();
    truncate_inline(&format!("{label}{suffix}"), budget)
}

fn apply_row_background(mut line: Line<'static>, background: Color) -> Line<'static> {
    for span in &mut line.spans {
        span.style.bg = Some(background);
        if span.style.fg.is_none() {
            span.style.fg = Some(Color::Black);
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    #[test]
    fn ch_panel_state_defaults() {
        let state = ChChainPanelState::new();
        assert!(!state.active);
        assert!(state.clerics.is_empty());
        assert!((state.cast_time_secs - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn chain_health_perfect_when_no_heals() {
        let state = ChChainPanelState::new();
        assert!((state.chain_health() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn chain_health_degrades_with_misses() {
        let mut state = ChChainPanelState::new();
        state.stats.total_heals = 10;
        state.stats.missed_heals = 3;
        let health = state.chain_health();
        assert!(health > 0.6 && health < 0.8);
    }

    #[test]
    fn move_cleric_up_down() {
        let mut state = ChChainPanelState::new();
        state.clerics = vec![
            ChainCleric {
                name: "Cleric1".into(),
                pid: 100,
                position: 1,
                timing_offset_ms: 0,
                cast_display: None,
                cast_state: CastState::Idle,
            },
            ChainCleric {
                name: "Cleric2".into(),
                pid: 101,
                position: 2,
                timing_offset_ms: 0,
                cast_display: None,
                cast_state: CastState::Idle,
            },
        ];
        state.selected = 1;
        state.move_up();
        assert_eq!(state.selected, 0);
        assert_eq!(state.clerics[0].name, "Cleric2");

        state.move_down();
        assert_eq!(state.selected, 1);
        assert_eq!(state.clerics[1].name, "Cleric2");
    }

    #[test]
    fn cycle_focus_wraps() {
        let mut state = ChChainPanelState::new();
        assert_eq!(state.focus, ChPanelFocus::ChainOrder);
        state.cycle_focus();
        assert_eq!(state.focus, ChPanelFocus::Timing);
        state.cycle_focus();
        assert_eq!(state.focus, ChPanelFocus::Target);
        state.cycle_focus();
        assert_eq!(state.focus, ChPanelFocus::Presets);
        state.cycle_focus();
        assert_eq!(state.focus, ChPanelFocus::ChainOrder);
    }

    #[test]
    fn chain_widget_renders_cast_strip_for_active_cleric() {
        let state = sample_state();

        let area = Rect::new(0, 0, 50, 12);
        let mut buf = Buffer::empty(area);
        ChChainWidget::new(&state).render(area, &mut buf);
        let rendered = buffer_contents(&buf, area);

        assert!(rendered.contains("Complete Heal") || rendered.contains("CH"));
        assert!(rendered.contains("Cast"));
    }

    #[test]
    fn chain_widget_compact_layout_merges_footer_and_shortens_header() {
        let state = sample_state();
        let area = Rect::new(0, 0, 48, 14);
        let mut buf = Buffer::empty(area);
        ChChainWidget::new(&state).render(area, &mut buf);
        let rendered = buffer_contents(&buf, area);

        assert!(rendered.contains("Tgt:"));
        assert!(rendered.contains("Timing / Health"));
        assert!(rendered.contains("CH"));
        assert!(!rendered.contains("Chain Health"));
    }

    #[test]
    fn chain_widget_medium_layout_keeps_full_sections() {
        let state = sample_state();
        let area = Rect::new(0, 0, 80, 22);
        let mut buf = Buffer::empty(area);
        ChChainWidget::new(&state).render(area, &mut buf);
        let rendered = buffer_contents(&buf, area);

        assert!(rendered.contains("Target:"));
        assert!(rendered.contains("Chain Health"));
        assert!(rendered.contains("Timing"));
        assert!(rendered.contains("Complete Heal") || rendered.contains("CH"));
    }

    fn sample_state() -> ChChainPanelState {
        let mut state = ChChainPanelState::new();
        state.target_name = String::from("Main Tank");
        state.target_id = 42;
        state.adaptive = true;
        state.stats.total_heals = 12;
        state.stats.missed_heals = 1;
        state.stats.late_casts = 2;
        state.clerics = vec![
            ChainCleric {
                name: String::from("Cleric1"),
                pid: 100,
                position: 1,
                timing_offset_ms: 0,
                cast_display: Some(CastDisplay::exact_progress(
                    "Complete Heal",
                    "CH",
                    0.5,
                    10.0,
                )),
                cast_state: CastState::Casting(0.5),
            },
            ChainCleric {
                name: String::from("Cleric2"),
                pid: 101,
                position: 2,
                timing_offset_ms: 150,
                cast_display: Some(CastDisplay::provisional(
                    "Heal Gem 2",
                    "Heal G2",
                    0.3,
                    Some(String::from("gem 2")),
                )),
                cast_state: CastState::Idle,
            },
            ChainCleric {
                name: String::from("Cleric3"),
                pid: 102,
                position: 3,
                timing_offset_ms: -75,
                cast_display: None,
                cast_state: CastState::Completed,
            },
        ];
        state
    }

    fn buffer_contents(buf: &Buffer, area: Rect) -> String {
        (area.top()..area.bottom())
            .map(|y| {
                (area.left()..area.right())
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
