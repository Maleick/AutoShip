//! CH (Complete Heal) Chain configuration and monitoring panel.
//!
//! Provides a dedicated UI for managing the cleric heal chain,
//! including chain ordering, timing, target selection, and real-time status.

use crate::tui::cast::CastDisplay;
use crate::tui::ui::widgets::render_cast_bar;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

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
            .title(" CH Chain Management ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.accent_color));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 5 || inner.width < 20 {
            return;
        }

        // Split into sections
        let layout = Layout::vertical([
            Constraint::Length(3), // Target & status header
            Constraint::Min(5),    // Chain member list
            Constraint::Length(3), // Timing config
            Constraint::Length(3), // Stats
        ])
        .split(inner);

        self.render_header(layout[0], buf);
        self.render_chain_list(layout[1], buf);
        self.render_timing(layout[2], buf);
        self.render_stats(layout[3], buf);
    }
}

impl ChChainWidget<'_> {
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let target_label = if self.state.target_name.is_empty() {
            format!("Target: (none) [ID: {}]", self.state.target_id)
        } else {
            format!(
                "Target: {} [ID: {}]",
                self.state.target_name, self.state.target_id
            )
        };

        let adaptive_label = if self.state.adaptive {
            Span::styled(" ADAPTIVE ", Style::default().fg(Color::Green))
        } else {
            Span::styled(" FIXED ", Style::default().fg(Color::Yellow))
        };

        let lines = vec![
            Line::from(vec![
                Span::styled(&target_label, Style::default().fg(Color::White)),
                Span::raw("  "),
                adaptive_label,
            ]),
            Line::from(Span::styled(
                format!(
                    "Chain: {} clerics, {:.1}s delay",
                    self.state.clerics.len(),
                    self.state.chain_delay_secs
                ),
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let para = Paragraph::new(lines);
        para.render(area, buf);
    }

    fn render_chain_list(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Chain Order ")
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

        let mut y = inner.y;
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
            let state_char = match cleric.cast_state {
                CastState::Idle => ("○", Color::DarkGray),
                CastState::Casting(pct) => {
                    if pct > 0.75 {
                        ("◕", Color::Green)
                    } else if pct > 0.25 {
                        ("◑", Color::Yellow)
                    } else {
                        ("◔", Color::White)
                    }
                }
                CastState::Completed => ("●", Color::Green),
                CastState::Missed => ("✗", Color::Red),
            }
            .0;
            let offset_text = if cleric.timing_offset_ms != 0 {
                format!(" {:+}ms", cleric.timing_offset_ms)
            } else {
                String::new()
            };
            let row_label = truncate_inline(
                &format!(
                    " {}. {}  {}{}",
                    cleric.position, cleric.name, state_char, offset_text
                ),
                inner.width as usize,
            );
            let row = Line::from(vec![Span::styled(row_label, base_style)]);
            buf.set_line(inner.x, y, &row, inner.width);
            y += 1;

            if let Some(cast_display) = &cleric.cast_display
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

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
        let mut state = ChChainPanelState::new();
        state.clerics.push(ChainCleric {
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
        });

        let area = Rect::new(0, 0, 50, 12);
        let mut buf = Buffer::empty(area);
        ChChainWidget::new(&state).render(area, &mut buf);
        let rendered = buffer_contents(&buf, area);

        assert!(rendered.contains("Complete Heal") || rendered.contains("CH"));
        assert!(rendered.contains("Cast"));
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

fn apply_row_background(mut line: Line<'static>, background: Color) -> Line<'static> {
    for span in &mut line.spans {
        span.style.bg = Some(background);
        if span.style.fg.is_none() {
            span.style.fg = Some(Color::Black);
        }
    }
    line
}

fn truncate_inline(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 2 {
        return text.chars().take(max_chars).collect();
    }

    let mut truncated: String = text.chars().take(max_chars - 2).collect();
    truncated.push('.');
    truncated.push('.');
    truncated
}
