//! TUI renderer — entry point and global chrome (header, status bar, help
//! overlay).
//!
//! Each screen lives in its own sub-module:
//! - [`roster`]      — character grid + health gauges + session stats
//! - [`spawns`]      — filterable spawn list + hex dump
//! - [`map`]         — zone map + named tracker
//! - [`groups`]      — per-group panels with buff timer columns
//! - [`navigation`]  — nav status + commands reference
//! - [`widgets`]     — shared helpers (`panel`, `themed_header_row`, colour fns
//!   …)

use std::time::{Duration, Instant};

pub mod ch_chain;
pub mod dps_bars;
pub mod economy_controls;
pub mod eq_internals;
pub mod explorer;
pub mod groups;
pub mod help;
pub mod map;
pub mod navigation;
pub mod orchestrator_panel;
pub mod packets;
pub mod patch_reconciliation;
pub mod spawns;
pub mod widgets;
pub mod zone_blocker_panel;
pub mod zone_status_panel;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    prelude::Widget,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    alerts::AlertSeverity,
    tui::{
        app::{ActivePanel, ActiveScreen, App, HelpFocus, ToastLevel},
        command::HelpSection,
        ui::widgets::{
            WidthClass, centered_popup, classify_width, line_width, spans_width, truncate_inline,
        },
    },
};

// ─── Entry point ─────────────────────────────────────────────────────────────

/// Top-level render function — applies outer margin then dispatches to the
/// active screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    // Apply a 1-cell horizontal margin so content never touches the terminal edges.
    let area = frame.area().inner(Margin {
        horizontal: 1,
        vertical: 0,
    });

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header + tab bar
            Constraint::Min(10),   // body
            Constraint::Length(3), // status bar
        ])
        .split(area);

    draw_header(frame, outer[0], app);

    match app.active_screen {
        ActiveScreen::Overview => roster::draw_roster(frame, outer[1], app),
        ActiveScreen::Tactical => map::draw_map_screen(frame, outer[1], app),
        ActiveScreen::Navigation => navigation::draw_navigation_screen(frame, outer[1], app),
        ActiveScreen::Debug => spawns::draw_debug_screen(frame, outer[1], app),
        ActiveScreen::PacketMonitor => packets::draw_packet_monitor(frame, outer[1], app),
        ActiveScreen::Economy => economy_controls::draw_economy_screen(frame, outer[1], app),
        ActiveScreen::Orchestrator => {
            orchestrator_panel::draw_orchestrator_screen(frame, outer[1], app)
        }
        ActiveScreen::Metrics => roster::draw_metrics_dashboard(frame, outer[1], app),
    }

    draw_status_bar(frame, outer[2], app);

    // ── Overlays (rendered last, on top) ──

    // Menu bar and dropdown
    if app.menu_state.active {
        use crate::tui::menu::{MenuBar, MenuDropdown};
        let menu_area = Rect::new(area.x, area.y, area.width, 1);
        frame.render_widget(
            MenuBar::new(&app.menu_state)
                .highlight_style(Style::default().fg(Color::Black).bg(app.theme.text_accent))
                .normal_style(Style::default().fg(app.theme.text_secondary)),
            menu_area,
        );
        let dropdown = MenuDropdown::new(&app.menu_state);
        let dd_rect = dropdown.dropdown_rect(menu_area);
        // Clamp dropdown to screen bounds
        let clamped = Rect::new(
            dd_rect.x,
            dd_rect.y,
            dd_rect
                .width
                .min(area.width.saturating_sub(dd_rect.x - area.x)),
            dd_rect
                .height
                .min(area.height.saturating_sub(dd_rect.y - area.y)),
        );
        frame.render_widget(Clear, clamped);
        frame.render_widget(
            MenuDropdown::new(&app.menu_state)
                .highlight_style(Style::default().fg(Color::Black).bg(app.theme.text_accent)),
            clamped,
        );
    }

    // Help overlay
    if app.help_visible {
        draw_help_overlay(frame, frame.area(), app);
    }

    // Help search panel overlay (#1117)
    if app.help_search_visible {
        help::draw_help_search_panel(frame, area, app);
    }

    // Config panel overlay
    if app.config_panel_state.active {
        use crate::tui::config_panel::ConfigPanelWidget;
        let popup_area = centered_popup(area, 92, 72, 96, 30, 96, 40, 1); // Fixed 96-wide
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            ConfigPanelWidget::new(&app.config_panel_state).accent_color(app.theme.text_accent),
            popup_area,
        );
    }

    // CH chain panel overlay
    if app.ch_chain_panel_state.active {
        use crate::tui::ui::ch_chain::ChChainWidget;
        let popup_area = centered_popup(area, 92, 72, 96, 28, 96, 36, 1); // Fixed 96-wide
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            ChChainWidget::new(&app.ch_chain_panel_state, &app.theme)
                .accent_color(app.theme.text_server),
            popup_area,
        );
    }

    // Wizard overlay
    if app.wizard_state.active {
        use crate::tui::wizard::WizardWidget;
        let popup_area = centered_popup(area, 92, 72, 96, 40, 96, 40, 1); // Fixed 96x40
        frame
            .buffer_mut()
            .set_style(area, Style::default().add_modifier(Modifier::DIM));
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            WizardWidget::new(&app.wizard_state).accent_color(app.theme.text_accent),
            popup_area,
        );
    }

    if app.alert_panel_visible {
        draw_alert_overlay(frame, area, app);
    }

    if app.diagnostics_visible {
        draw_diagnostics_overlay(frame, area, app);
    }

    // Toast notification (bottom-right centered overlay, 62 wide, 4 tall with borders)
    if let Some(toast) = app.toast.as_ref() {
        let t = &app.theme;
        let toast_area = centered_popup(area, 45, 8, 62, 5, 62, 5, 1); // Fixed 62x5
        let border_color = Style::default().fg(t.hp_high); // Green border

        // Split message on newline or bullet separator for 2-line display
        let lines: Vec<&str> = toast.message.split('\n').collect();
        let line1 = lines.first().map(|s| s.as_ref()).unwrap_or("");
        let line2 = lines.get(1).map(|s| s.as_ref()).unwrap_or("");

        // Truncate each line to fit in the 62-wide panel (accounting for borders and padding)
        let available_width = (62 - 4) as usize; // -4 for borders and padding
        let line1_text = truncate_inline(line1, available_width);
        let line2_text = if line2.is_empty() {
            String::new()
        } else {
            truncate_inline(line2, available_width)
        };

        frame.render_widget(Clear, toast_area);

        let title = " Notice ";
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_color)
            .border_type(t.border_type);

        let inner = block.inner(toast_area);
        block.render(toast_area, frame.buffer_mut());

        // Render lines within the inner area
        let lines_to_render = vec![
            Line::from(Span::styled(
                format!("● {}", line1_text),
                Style::default().fg(t.hp_high).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!("  {}", line2_text),
                Style::default().fg(t.text_muted),
            )),
            Line::from(Span::styled(
                toast_progress_bar(toast, app.refresh_rate_ms, inner.width as usize),
                Style::default().fg(t.hp_high),
            )),
        ];

        let para = Paragraph::new(lines_to_render);
        para.render(inner, frame.buffer_mut());
    }
}

fn toast_progress_bar(
    toast: &crate::tui::app::Toast,
    refresh_rate_ms: u64,
    width: usize,
) -> String {
    if width == 0 {
        return String::new();
    }

    let total = Duration::from_millis(refresh_rate_ms.saturating_mul(toast.ttl_ticks));
    if total.is_zero() {
        return "░".repeat(width);
    }

    let remaining = toast.expires_at.saturating_duration_since(Instant::now());
    let fraction = (remaining.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0);
    let filled = fraction * width as f64;
    let solid = filled.floor() as usize;
    let has_partial = solid < width && filled > solid as f64;

    let mut bar = String::with_capacity(width);
    bar.push_str(&"█".repeat(solid.min(width)));
    if has_partial {
        bar.push('▓');
    }
    let used = solid.min(width) + usize::from(has_partial);
    bar.push_str(&"░".repeat(width.saturating_sub(used)));
    bar
}

// ─── Header ──────────────────────────────────────────────────────────────────

fn push_segment_if_fits(
    spans: &mut Vec<Span<'static>>,
    mut segment: Vec<Span<'static>>,
    max_width: usize,
) -> bool {
    let candidate_width = spans_width(spans) + spans_width(&segment);
    if candidate_width > max_width {
        return false;
    }
    spans.append(&mut segment);
    true
}

fn header_tab_label(screen: ActiveScreen, width_class: WidthClass) -> &'static str {
    match width_class {
        WidthClass::Wide => screen.label(),
        WidthClass::Medium => match screen {
            ActiveScreen::Overview => "Teth",
            ActiveScreen::Tactical => "Cart",
            ActiveScreen::Navigation => "Way",
            ActiveScreen::Debug => "Ora",
            ActiveScreen::PacketMonitor => "Aeth",
            ActiveScreen::Economy => "Coin",
            ActiveScreen::Orchestrator => "Gate",
            ActiveScreen::Metrics => "Metr",
        },
        WidthClass::Narrow => match screen {
            ActiveScreen::Overview => "1",
            ActiveScreen::Tactical => "2",
            ActiveScreen::Navigation => "3",
            ActiveScreen::Debug => "4",
            ActiveScreen::PacketMonitor => "5",
            ActiveScreen::Economy => "6",
            ActiveScreen::Orchestrator => "7",
            ActiveScreen::Metrics => "8",
        },
    }
}

fn build_header_tabs(app: &App, width_class: WidthClass) -> Line<'static> {
    let t = &app.theme;
    let mut spans = Vec::new();
    for (index, screen) in ActiveScreen::ALL.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(""));
        }
        let label = header_tab_label(*screen, width_class);
        let idx = index + 1;
        if *screen == app.active_screen {
            // Active tab: [inverse label] with magenta brackets
            spans.push(Span::styled(
                "[",
                Style::default().fg(t.border_primary.fg.unwrap_or(Color::Magenta)),
            ));
            spans.push(Span::styled(
                format!(" {idx} {label} "),
                Style::default()
                    .fg(t.text_bright)
                    .bg(t.row_selected_bg)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                "]",
                Style::default().fg(t.border_primary.fg.unwrap_or(Color::Magenta)),
            ));
        } else {
            // Inactive tab: [label] with muted brackets, secondary label
            spans.push(Span::styled("[", Style::default().fg(t.text_muted)));
            spans.push(Span::styled(
                format!(" {idx} {label} "),
                Style::default().fg(t.text_secondary),
            ));
            spans.push(Span::styled("]", Style::default().fg(t.text_muted)));
        }
    }
    Line::from(spans)
}

fn build_header_meta(app: &App, width_class: WidthClass, max_width: usize) -> Vec<Span<'static>> {
    let t = &app.theme;
    let client_count = app.clients.len();

    // Left-side metadata segments, built from the design mock
    let mut spans = Vec::new();

    // 1. Client count: "{count}x EQ" in cyan + secondary
    if client_count > 0 {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::styled(
                    format!("{}x", client_count),
                    Style::default()
                        .fg(t.text_accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" EQ", Style::default().fg(t.text_secondary)),
            ],
            max_width,
        );
    } else {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![Span::styled(
                "Not attached",
                Style::default().fg(t.text_secondary),
            )],
            max_width,
        );
    }

    // 2. Selected client: "1/6 Name" separator + index/total in bright + name in highlight
    if let Some(client) = app.active_client() {
        let name = client.local_player.as_ref().map_or_else(
            || String::from("???"),
            |player| app.redact_name(&player.displayed_name).into_owned(),
        );
        let selected_name_budget = match width_class {
            WidthClass::Narrow => 10,
            WidthClass::Medium => 16,
            WidthClass::Wide => 24,
        };
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::styled(" │ ", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!("{}/{}", app.selected_client + 1, client_count.max(1)),
                    Style::default()
                        .fg(t.text_bright)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
                Span::styled(
                    truncate_inline(&name, selected_name_budget),
                    Style::default().fg(t.text_highlight),
                ),
            ],
            max_width,
        );
    }

    // 3. Focus group: "G2 Fear Core" separator + id in accent + label in secondary
    let group_budget = match width_class {
        WidthClass::Narrow => 10,
        WidthClass::Medium => 14,
        WidthClass::Wide => 18,
    };
    let _ = push_segment_if_fits(
        &mut spans,
        vec![
            Span::styled(" │ ", Style::default().fg(t.text_muted)),
            Span::styled(app.group_focus_label(), Style::default().fg(t.text_accent)),
        ],
        max_width,
    );

    // 4. Server: separator + server name in server color (bold)
    let server_budget = match width_class {
        WidthClass::Narrow => 8,
        WidthClass::Medium => 12,
        WidthClass::Wide => 18,
    };
    let _ = push_segment_if_fits(
        &mut spans,
        vec![
            Span::styled(" │ ", Style::default().fg(t.text_muted)),
            Span::styled(
                truncate_inline(app.display_server(), server_budget),
                Style::default()
                    .fg(t.text_server)
                    .add_modifier(Modifier::BOLD),
            ),
        ],
        max_width,
    );

    // 5. Zone: separator + zone name in bright
    let zone_budget = match width_class {
        WidthClass::Narrow => 10,
        WidthClass::Medium => 14,
        WidthClass::Wide => 22,
    };
    let zone_label = app.active_client().map_or_else(
        || String::from("No Zone"),
        |client| {
            if client.zone_name.is_empty() {
                String::from("Unknown Zone")
            } else {
                client.zone_name.clone()
            }
        },
    );
    let _ = push_segment_if_fits(
        &mut spans,
        vec![
            Span::styled(" │ ", Style::default().fg(t.text_muted)),
            Span::styled(
                truncate_inline(&zone_label, zone_budget),
                Style::default().fg(t.text_bright),
            ),
        ],
        max_width,
    );

    spans
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    // Header layout: 3 lines total
    // Line 1: top border with title
    // Line 2: left meta + padding + right tabs
    // Line 3: bottom border
    let lines = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let width = area.width as usize;
    let width_class = classify_width(area.width);

    // ─── Line 1: Top border with title inset ───
    // Wordmark sourced from `tui::branding::WORDMARK` — kept in lockstep with
    // the web frontend's "TEXTQUEST" Cinzel display treatment.
    let title = format!(
        "{} v{}",
        crate::tui::branding::WORDMARK,
        crate::tui::branding::VERSION
    );
    let border_char = "─";
    let left_border = "╭";
    let right_border = "╮";
    let left_lead = format!("{}{}{}", left_border, border_char, border_char);
    let right_tail = format!("{}{}", border_char, right_border);
    let title_styled = format!(" {} ", title);
    let title_width = title_styled.len();

    let left_lead_width = left_border.len() + 2;
    let right_tail_width = 1 + right_border.len();
    let filler_width = width
        .saturating_sub(left_lead_width)
        .saturating_sub(title_width)
        .saturating_sub(right_tail_width);
    let filler = border_char.repeat(filler_width);

    let top_line = format!("{}{}{}{}", left_lead, title_styled, filler, right_tail);
    let top_spans = vec![Span::styled(
        top_line,
        Style::default()
            .fg(t.border_primary.fg.unwrap_or(Color::Magenta))
            .add_modifier(Modifier::BOLD),
    )];
    frame.render_widget(Paragraph::new(Line::from(top_spans)), lines[0]);

    // ─── Line 2: Meta + Tabs ───
    let tabs = build_header_tabs(app, width_class);
    let tabs_width = line_width(&tabs).min(width) as u16;
    let left_meta = build_header_meta(app, width_class, width);

    let left_width = spans_width(&left_meta);
    let gap_width = width
        .saturating_sub(left_width)
        .saturating_sub(tabs_width as usize);

    let mut mid_spans = vec![Span::styled(
        "│",
        Style::default()
            .fg(t.border_primary.fg.unwrap_or(Color::Magenta))
            .add_modifier(Modifier::BOLD),
    )];
    mid_spans.extend(left_meta);
    if gap_width > 0 {
        mid_spans.push(Span::raw(" ".repeat(gap_width)));
    }
    // Add tabs from the line
    mid_spans.extend(tabs.spans);
    mid_spans.push(Span::styled(
        "│",
        Style::default()
            .fg(t.border_primary.fg.unwrap_or(Color::Magenta))
            .add_modifier(Modifier::BOLD),
    ));

    frame.render_widget(Paragraph::new(Line::from(mid_spans)), lines[1]);

    // ─── Line 3: Bottom border ───
    let bottom_border = "╰";
    let bottom_end = "╯";
    let bottom_fill = border_char.repeat(width.saturating_sub(2));
    let bottom_line = format!("{}{}{}", bottom_border, bottom_fill, bottom_end);
    let bot_spans = vec![Span::styled(
        bottom_line,
        Style::default()
            .fg(t.border_primary.fg.unwrap_or(Color::Magenta))
            .add_modifier(Modifier::BOLD),
    )];
    frame.render_widget(Paragraph::new(Line::from(bot_spans)), lines[2]);
}

// ─── Status bar ──────────────────────────────────────────────────────────────

fn status_hints(app: &App, width_class: WidthClass) -> &'static [(&'static str, &'static str)] {
    if app.active_screen == ActiveScreen::Metrics {
        match width_class {
            WidthClass::Narrow => &[
                ("1-8", "screen"),
                ("Tab", "view"),
                ("↑↓", "row"),
                ("S", "sort"),
                ("?", "help"),
            ],
            WidthClass::Medium => &[
                ("1-8", "screen"),
                ("Tab", "view"),
                ("Shift+Tab", "prev"),
                ("↑↓", "row"),
                ("Enter", "detail"),
                ("S", "sort"),
                ("F", "fleet"),
                ("?", "help"),
            ],
            WidthClass::Wide => &[
                ("1-8", "screen"),
                ("Tab", "next view"),
                ("Shift+Tab", "prev view"),
                ("↑↓", "scroll"),
                ("Enter", "detail"),
                ("S", "sort"),
                ("Space", "select"),
                ("F", "fleet/individual"),
                ("?", "help"),
            ],
        }
    } else if app.active_screen == ActiveScreen::Tactical {
        match width_class {
            WidthClass::Narrow => &[
                ("1-8", "screen"),
                ("+/-", "zoom"),
                ("n", "mesh"),
                ("v", "view"),
                ("?", "help"),
            ],
            WidthClass::Medium => &[
                ("1-8", "screen"),
                ("+/-", "zoom"),
                ("g", "geo"),
                ("s", "spawns"),
                ("w", "paths"),
                ("x", "mesh"),
                ("n", "navmesh"),
                ("v", "view"),
                ("?", "help"),
            ],
            WidthClass::Wide => &[
                ("1-8", "screen"),
                ("Tab", "pane"),
                ("+/-", "zoom"),
                ("g", "geo"),
                ("s", "spawns"),
                ("w", "paths"),
                ("x", "mesh"),
                ("n", "navmesh"),
                ("l", "labels"),
                ("v", "view"),
                ("</>", "depth"),
                ("Home", "reset"),
                ("?", "help"),
            ],
        }
    } else {
        match width_class {
            WidthClass::Narrow => &[
                ("1-8", "screen"),
                ("Tab", "pane"),
                ("[ ]", "client"),
                ("/", "search"),
                ("F8", "alerts"),
                ("^D", "diag"),
                ("g/v", "sect"),
                ("?", "help"),
            ],
            WidthClass::Medium => &[
                ("1-8", "screen"),
                ("Shift+1-6", "group"),
                ("Tab", "pane"),
                ("[ ]", "client"),
                ("F8", "alerts"),
                ("Ctrl+D", "diag"),
                ("g/v", "sections"),
                ("z", "collapse"),
                ("/", "search"),
                ("f", "filter"),
                ("?", "help"),
            ],
            WidthClass::Wide => &[
                ("1-8", "screen"),
                ("Shift+1-6", "group"),
                ("Tab", "pane"),
                ("[ ]", "client"),
                ("F8", "alerts"),
                ("Ctrl+D", "diag"),
                ("g/v", "sections"),
                ("z", "collapse"),
                ("/", "search"),
                ("f", "filter"),
                ("T", "theme"),
                ("F10", "menu"),
                ("?", "help"),
            ],
        }
    }
}

fn build_status_left(app: &App, width_class: WidthClass, max_width: usize) -> Vec<Span<'static>> {
    let t = &app.theme;
    let mut spans = vec![Span::raw(" ")];

    // SCREEN_NAME in bright bold
    let screen_name = match app.active_screen {
        ActiveScreen::Overview => "SOUL TETHERS",
        ActiveScreen::Tactical => "CARTOGRAPHY",
        ActiveScreen::Navigation => "WAYPATH",
        ActiveScreen::Debug => "ORACLE",
        ActiveScreen::PacketMonitor => "AETHERGRAM",
        ActiveScreen::Economy => "COINMARK",
        ActiveScreen::Orchestrator => "THIRD GATE",
        ActiveScreen::Metrics => "METRICS",
    };
    spans.push(Span::styled(
        screen_name,
        Style::default()
            .fg(t.text_bright)
            .add_modifier(Modifier::BOLD),
    ));

    // " ▸ " in muted
    spans.push(Span::styled(" ▸ ", Style::default().fg(t.text_muted)));

    // Keybind hints in secondary
    for (key, desc) in status_hints(app, width_class) {
        let segment = widgets::keybinding_hint(key, desc, t);
        if !push_segment_if_fits(&mut spans, segment, max_width) {
            break;
        }
    }
    spans
}

fn build_status_right(app: &App, width_class: WidthClass, max_width: usize) -> Vec<Span<'static>> {
    let t = &app.theme;
    let mut spans = Vec::new();

    // Mode pill: status badge with accent color (magenta for HUNT, cyan for CAMP)
    let mode_str = format!("{}", app.operating_mode);
    let pill = widgets::status_pill(&mode_str, t);
    let _ = push_segment_if_fits(
        &mut spans,
        pill,
        max_width,
    );

    // Separator and MA
    let ma_label = app.main_assist.as_deref().map(|name| {
        truncate_inline(
            name,
            if width_class == WidthClass::Wide {
                12
            } else {
                8
            },
        )
    });
    if let Some(ma) = ma_label {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::styled(" │ ", Style::default().fg(t.text_muted)),
                Span::styled("MA ", Style::default().fg(t.text_secondary)),
                Span::styled(ma, Style::default().fg(t.text_bright)),
            ],
            max_width,
        );
    }

    // MT
    let mt_label = app.main_tank.as_deref().map(|name| {
        truncate_inline(
            name,
            if width_class == WidthClass::Wide {
                12
            } else {
                8
            },
        )
    });
    if let Some(mt) = mt_label {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::styled(" │ ", Style::default().fg(t.text_muted)),
                Span::styled("MT ", Style::default().fg(t.text_secondary)),
                Span::styled(mt, Style::default().fg(t.text_bright)),
            ],
            max_width,
        );
    }

    // CH chain: "CH {members}x@{interval}s {adaptive}"
    let ch_label = app
        .ch_chain_status
        .as_ref()
        .map(|chain| format!("{}x@{:.1}s", chain.members, chain.interval_secs,));
    if let Some(ch) = ch_label {
        let adaptive_char = if app
            .ch_chain_status
            .as_ref()
            .map(|c| c.is_adaptive)
            .unwrap_or(false)
        {
            "A"
        } else {
            "·"
        };
        let adaptive_style = if app
            .ch_chain_status
            .as_ref()
            .map(|c| c.is_adaptive)
            .unwrap_or(false)
        {
            Style::default().fg(t.hp_high) // green for adaptive
        } else {
            Style::default().fg(t.text_muted) // muted for non-adaptive
        };
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::styled(" │ ", Style::default().fg(t.text_muted)),
                Span::styled("CH ", Style::default().fg(t.text_accent)),
                Span::styled(ch, Style::default().fg(t.text_bright)),
                Span::raw(" "),
                Span::styled(adaptive_char, adaptive_style),
            ],
            max_width,
        );
    }

    // Focus group pill: "[G2]" with inverse styling
    if app.active_group.is_some() {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::styled(" │ ", Style::default().fg(t.text_muted)),
                Span::styled(
                    "[",
                    Style::default().fg(t.border_primary.fg.unwrap_or(Color::Magenta)),
                ),
                Span::styled(
                    format!(" {} ", app.group_focus_label()),
                    Style::default()
                        .fg(t.text_bright)
                        .bg(t.row_selected_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "]",
                    Style::default().fg(t.border_primary.fg.unwrap_or(Color::Magenta)),
                ),
            ],
            max_width,
        );
    }

    // Alerts badge
    let unread_alerts = app.unread_alert_count();
    let _ = push_segment_if_fits(
        &mut spans,
        vec![
            Span::styled(" │ ", Style::default().fg(t.text_muted)),
            Span::styled("Alerts ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format!("{}", unread_alerts),
                if unread_alerts > 0 {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD) // amber
                } else {
                    Style::default().fg(t.text_muted)
                },
            ),
        ],
        max_width,
    );

    // Theme name in muted (right-aligned)
    if width_class != WidthClass::Narrow {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::styled(" │ ", Style::default().fg(t.text_muted)),
                Span::styled(app.theme_kind.label(), Style::default().fg(t.text_muted)),
            ],
            max_width,
        );
    }

    spans
}

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    // Split area into rule (1 line) + status (1 line) + padding (1 line)
    let lines = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    // Line 1: Dashed rule
    let rule_width = area.width as usize;
    let dashed_rule = "─".repeat(rule_width);
    let rule_spans = vec![Span::styled(dashed_rule, Style::default().fg(t.text_muted))];
    frame.render_widget(Paragraph::new(Line::from(rule_spans)), lines[0]);

    // Line 2: Status bar content
    // Command mode: full-width input line with syntax hint
    if app.cmd_state.command_mode {
        let chars: Vec<char> = app.cmd_state.command_buffer.chars().collect();
        let cursor = app.cmd_state.cursor.min(chars.len());
        let before: String = chars.iter().take(cursor).collect();
        let at_cursor = chars.get(cursor).copied().unwrap_or(' ');
        let after: String = chars
            .iter()
            .skip(cursor + usize::from(cursor < chars.len()))
            .collect();
        let mut spans = vec![
            Span::styled(": ", t.statusbar_cmd),
            Span::styled(before, t.statusbar_cmd),
            Span::styled(
                at_cursor.to_string(),
                Style::default().fg(Color::Black).bg(t.text_bright),
            ),
            Span::styled(after, t.statusbar_cmd),
        ];

        if app.cmd_state.command_buffer.trim().is_empty() {
            spans.push(Span::styled(
                "  Type a command  Tab complete  Up/Down history  Esc cancel",
                Style::default().fg(t.text_muted),
            ));
        } else if let Some(hint) =
            crate::tui::app::command_syntax_hint(&app.cmd_state.command_buffer)
        {
            let example = crate::tui::app::command_example_hint(&app.cmd_state.command_buffer)
                .unwrap_or(hint);
            spans.push(Span::styled("  |  ", Style::default().fg(t.text_muted)));
            spans.push(Span::styled(
                truncate_inline(hint, 42),
                Style::default().fg(t.text_accent),
            ));
            spans.push(Span::styled("  eg ", Style::default().fg(t.text_muted)));
            spans.push(Span::styled(
                truncate_inline(&format!(":{}", example), 28),
                Style::default().fg(t.text_secondary),
            ));
        }

        frame.render_widget(
            Paragraph::new(Line::from(spans)).block(widgets::panel("", t.border_active, t)),
            lines[1],
        );
        return;
    }

    let width_class = classify_width(area.width);
    let right_spans = build_status_right(app, width_class, area.width as usize);
    let right_width = (spans_width(&right_spans) as u16 + 2)
        .max(12)
        .min(area.width.saturating_sub(12));
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(right_width)])
        .split(lines[1]);
    let left_spans = build_status_left(app, width_class, cols[0].width.saturating_sub(2) as usize);

    frame.render_widget(
        Paragraph::new(Line::from(left_spans)).block(widgets::panel("", t.border_dim, t)),
        cols[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(right_spans)).block(widgets::panel("", t.border_dim, t)),
        cols[1],
    );
}

fn alert_severity_style(t: &crate::tui::theme::Theme, severity: AlertSeverity) -> Style {
    match severity {
        AlertSeverity::Critical => Style::default()
            .fg(Color::White)
            .bg(t.hp_low)
            .add_modifier(Modifier::BOLD),
        AlertSeverity::Warning => Style::default()
            .fg(Color::Black)
            .bg(t.text_highlight)
            .add_modifier(Modifier::BOLD),
        AlertSeverity::Info => Style::default()
            .fg(Color::Black)
            .bg(t.text_accent)
            .add_modifier(Modifier::BOLD),
    }
}

fn draw_alert_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let popup = centered_popup(area, 92, 72, 96, 28, 96, 36, 1); // Fixed 96x28
    let t = &app.theme;
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(7),
        ])
        .split(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![Span::styled(
                format!("Alert Feed · {} unread", app.unread_alert_count()),
                Style::default()
                    .fg(t.text_bright)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(Span::styled(
                "↑↓ select · a acknowledge · A ack all · F8 close",
                t.statusbar_dim,
            )),
        ])
        .block(widgets::panel("", t.border_warn, t)),
        sections[0],
    );

    let list_body_height = sections[1].height.saturating_sub(2) as usize;
    let visible_height = list_body_height.max(1);
    let start = app
        .alert_selected
        .saturating_sub(visible_height.saturating_sub(1));
    let end = (start + visible_height).min(app.alert_history.len());
    let mut lines = Vec::new();

    if app.alert_history.is_empty() {
        lines.push(Line::from(Span::styled(
            "No operational alerts recorded yet.",
            t.statusbar_dim,
        )));
    } else {
        for (idx, alert) in app.alert_history[start..end].iter().enumerate() {
            let absolute_idx = start + idx;

            // Severity-colored dot: ● for unread, ○ for acknowledged
            let (dot, dot_style) = if alert.unread() {
                let severity_color = match alert.severity {
                    AlertSeverity::Critical => t.hp_low,        // red
                    AlertSeverity::Warning => t.text_highlight, // amber
                    AlertSeverity::Info => t.text_accent,       // cyan
                };
                (
                    "●",
                    Style::default()
                        .fg(severity_color)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ("○", Style::default().fg(t.text_muted))
            };

            let timestamp = alert.created_at.split('T').nth(1).unwrap_or("--:--:--");
            let message = truncate_inline(
                &alert.message,
                sections[1].width.saturating_sub(40) as usize,
            );

            // Line 1: {dot} {Severity} {timestamp} {message}
            let line1 = Line::from(vec![
                Span::styled(format!("{} ", dot), dot_style),
                Span::styled(
                    format!("{:<10}", alert.severity.as_str().to_uppercase()),
                    if alert.unread() {
                        Style::default()
                            .fg(t.text_bright)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(t.text_secondary)
                    },
                ),
                Span::styled(format!("{:<10}", timestamp), t.text_secondary),
                Span::styled(message, Style::default().fg(t.text_bright)),
            ]);
            lines.push(line1);

            // Line 2: kind and source in muted
            let line2 = Line::from(vec![
                Span::styled("  ", t.text_muted),
                Span::styled(format!("kind={} ", alert.kind.display_name()), t.text_muted),
                Span::styled(
                    format!("source={}", alert.kind.display_name()),
                    t.text_muted,
                ),
            ]);
            lines.push(line2);
        }
    }

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        sections[1],
    );

    let detail_lines = if let Some(alert) = app.alert_history.get(app.alert_selected) {
        vec![
            Line::from(vec![
                Span::styled("Kind: ", t.statusbar_dim),
                Span::styled(
                    alert.kind.display_name(),
                    Style::default().fg(t.text_bright),
                ),
                Span::raw("   "),
                Span::styled("Created: ", t.statusbar_dim),
                Span::styled(
                    alert.created_at.clone(),
                    Style::default().fg(t.text_secondary),
                ),
            ]),
            Line::from(vec![
                Span::styled("Source: ", t.statusbar_dim),
                Span::styled(
                    alert.source.clone().unwrap_or_else(|| String::from("n/a")),
                    Style::default().fg(t.text_secondary),
                ),
                Span::raw("   "),
                Span::styled("Actor: ", t.statusbar_dim),
                Span::styled(
                    alert.actor.clone().unwrap_or_else(|| String::from("n/a")),
                    Style::default().fg(t.text_secondary),
                ),
            ]),
            Line::from(vec![
                Span::styled("Zone: ", t.statusbar_dim),
                Span::styled(
                    alert.zone.clone().unwrap_or_else(|| String::from("n/a")),
                    Style::default().fg(t.text_secondary),
                ),
                Span::raw("   "),
                Span::styled("Ack: ", t.statusbar_dim),
                Span::styled(
                    alert
                        .acknowledged_by
                        .clone()
                        .unwrap_or_else(|| String::from("unread")),
                    Style::default().fg(t.text_secondary),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                alert.message.clone(),
                Style::default().fg(t.text_bright),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(
            "Select an alert to inspect details.",
            t.statusbar_dim,
        ))]
    };

    frame.render_widget(
        Paragraph::new(detail_lines)
            .block(widgets::panel("Details", t.border_dim, t))
            .wrap(Wrap { trim: false }),
        sections[2],
    );
}

fn draw_diagnostics_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let popup = centered_popup(area, 86, 78, 92, 24, 92, 34, 1);
    frame.render_widget(Clear, popup);

    let focused = app.focused_pid_count();
    let connected = app.clients.len();
    let memory = crate::metrics::sample_process_memory_bytes(std::process::id())
        .map(|bytes| format!("{} MB", bytes / 1024 / 1024))
        .unwrap_or_else(|| String::from("unavailable"));
    let stuck_count = app
        .nav_state
        .nav_statuses
        .values()
        .filter(|status| status.status.is_stuck())
        .count();
    let blocker_count: usize = app
        .nav_state
        .nav_statuses
        .values()
        .map(|status| status.blockers.len() + usize::from(status.failure_reason.is_some()))
        .sum();
    let last_errors = app.diagnostics.recent_errors.len();
    let health = if connected == 0 {
        ("attention", t.hp_low)
    } else if stuck_count > 0 || app.diagnostics.ipc_failures > 0 {
        ("degraded", t.text_highlight)
    } else {
        ("healthy", t.hp_high)
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("System health: ", Style::default().fg(t.text_secondary)),
            Span::styled(
                health.0,
                Style::default().fg(health.1).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "    Close: Ctrl+D / q / Esc",
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("DLL connection: ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format!("{connected} connected, {focused} focused"),
                Style::default().fg(t.text_bright),
            ),
        ]),
        Line::from(vec![
            Span::styled("IPC throughput: ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format!(
                    "{} sent, {} failed",
                    app.diagnostics.ipc_commands_sent, app.diagnostics.ipc_failures
                ),
                Style::default().fg(t.text_bright),
            ),
        ]),
        Line::from(vec![
            Span::styled("Memory usage: ", Style::default().fg(t.text_secondary)),
            Span::styled(memory, Style::default().fg(t.text_bright)),
            Span::styled(
                format!(
                    "    tracked state: {} spawns, {} chat events",
                    app.spawns.len(),
                    app.chat_events.len()
                ),
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::from(vec![
            Span::styled("Stuck triggers: ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format!("{stuck_count} stuck clients, {blocker_count} blockers"),
                Style::default().fg(if stuck_count > 0 { t.hp_low } else { t.hp_high }),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Last 10 errors", Style::default().fg(t.text_bright)),
            Span::styled(
                format!(" ({last_errors} recorded, scroll j/k or arrows)"),
                Style::default().fg(t.text_muted),
            ),
        ]),
    ];

    if app.diagnostics.recent_errors.is_empty() {
        lines.push(Line::from(Span::styled(
            "No warnings or errors recorded this session.",
            Style::default().fg(t.text_muted),
        )));
    } else {
        for error in app
            .diagnostics
            .recent_errors
            .iter()
            .skip(app.diagnostics_scroll)
            .take(4)
        {
            let level_color = match error.level {
                ToastLevel::Error => t.hp_low,
                ToastLevel::Warning => t.text_highlight,
                ToastLevel::Info | ToastLevel::Success => t.text_secondary,
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("[{}] ", error.code),
                    Style::default()
                        .fg(level_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    truncate_inline(&error.message, popup.width.saturating_sub(18) as usize),
                    Style::default().fg(t.text_normal),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Suggestion: ", Style::default().fg(t.text_secondary)),
                Span::styled(
                    truncate_inline(&error.suggestion, popup.width.saturating_sub(18) as usize),
                    Style::default().fg(t.text_muted),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Source: ", Style::default().fg(t.text_secondary)),
                Span::styled(error.source.clone(), Style::default().fg(t.text_muted)),
                Span::styled(
                    if error.debug_trace.is_some() {
                        "    debug trace captured"
                    } else {
                        "    release build"
                    },
                    Style::default().fg(t.text_muted),
                ),
            ]));
        }
    }

    let block = Block::default()
        .title(" Diagnostics ")
        .borders(Borders::ALL)
        .border_type(t.border_type)
        .border_style(Style::default().fg(t.border_active));
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        popup,
    );
}

// ─── Help overlay
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct HelpLine {
    marker: Option<HelpFocus>,
    line: Line<'static>,
}

enum HelpCell {
    Heading(String),
    KeyValue(String, String),
    Text(String),
}

struct HelpRow {
    marker: Option<HelpFocus>,
    cell: HelpCell,
}

fn help_row(marker: Option<HelpFocus>, cell: HelpCell) -> HelpRow {
    HelpRow { marker, cell }
}

fn push_heading(rows: &mut Vec<HelpRow>, marker: Option<HelpFocus>, title: impl Into<String>) {
    rows.push(help_row(marker, HelpCell::Heading(title.into())));
    rows.push(help_row(None, HelpCell::Text(String::new())));
}

fn push_kv(
    rows: &mut Vec<HelpRow>,
    marker: Option<HelpFocus>,
    key: impl Into<String>,
    value: impl Into<String>,
) {
    rows.push(help_row(
        marker,
        HelpCell::KeyValue(key.into(), value.into()),
    ));
}

fn build_help_outline(app: &App) -> Vec<HelpRow> {
    let mut rows = Vec::new();

    rows.push(help_row(
        None,
        HelpCell::Text(format!("Active: {}", app.active_screen.label())),
    ));
    rows.push(help_row(None, HelpCell::Text(String::new())));

    // Overview screen
    {
        let marker = if app.active_screen == ActiveScreen::Overview {
            Some(HelpFocus::Command("overview"))
        } else {
            None
        };
        push_heading(&mut rows, marker, "Overview — Soul Tethers Controls");
        push_kv(&mut rows, None, "j/k", "Navigate the client roster");
        push_kv(
            &mut rows,
            None,
            "Enter",
            "Expand selected character details",
        );
        push_kv(&mut rows, None, "g", "Toggle group roster section");
        push_kv(&mut rows, None, "v", "Toggle scope / filters section");
        push_kv(&mut rows, None, "z", "Collapse or expand focused section");
        rows.push(help_row(None, HelpCell::Text(String::new())));
        push_heading(&mut rows, None, "Quick Commands");
        push_kv(&mut rows, None, "e", "Engage combat on current target");
        push_kv(&mut rows, None, "d", "Disengage from combat");
        push_kv(&mut rows, None, "l", "Loot nearby corpses");
        push_kv(&mut rows, None, "r", "Repeat last command");
        rows.push(help_row(None, HelpCell::Text(String::new())));
    }

    // Tactical screen
    {
        let marker = if app.active_screen == ActiveScreen::Tactical {
            Some(HelpFocus::Command("tactical"))
        } else {
            None
        };
        push_heading(&mut rows, marker, "Tactical — Cartography Controls");
        push_kv(&mut rows, None, "?", "Toggle this help overlay");
        push_kv(&mut rows, None, "+ / -", "Zoom in / out");
        push_kv(&mut rows, None, "Arrows", "Pan the map viewport");
        push_kv(&mut rows, None, "Home", "Center on player (local view)");
        push_kv(&mut rows, None, "End", "Fit the full zone / global view");
        push_kv(
            &mut rows,
            None,
            "v",
            "Cycle viewport: Auto / Local / Global",
        );
        push_kv(
            &mut rows,
            None,
            "Ctrl+A / Ctrl+L / Ctrl+G",
            "Set Auto / Local / Global view directly",
        );
        push_kv(&mut rows, None, "m", "Maximize / restore map panel");
        push_kv(
            &mut rows,
            None,
            "Shift+I",
            "Show zone/map stats in the status lane",
        );
        push_kv(
            &mut rows,
            None,
            "< / >",
            "Adjust Z-depth slice (height filter)",
        );
        rows.push(help_row(None, HelpCell::Text(String::new())));
        push_heading(&mut rows, None, "Map Layers");
        push_kv(
            &mut rows,
            None,
            "g",
            "Geometry: zone walls, floors, and boundaries from map files",
        );
        push_kv(
            &mut rows,
            None,
            "s",
            "Spawns: NPC/PC markers showing mob and player positions",
        );
        push_kv(
            &mut rows,
            None,
            "w",
            "Paths: navigation waypoint routes your characters follow",
        );
        push_kv(
            &mut rows,
            None,
            "x / Shift+N",
            "Mesh: navigation mesh walkable-area overlay for pathfinding",
        );
        push_kv(
            &mut rows,
            None,
            "l",
            "Labels: text POI markers (zone connections, banks, NPCs)",
        );
        push_kv(
            &mut rows,
            None,
            "a",
            "Annotations: Brewall layer-2 extras (compass roses, grid marks)",
        );
        rows.push(help_row(None, HelpCell::Text(String::new())));
        push_heading(&mut rows, None, "Map Filters");
        push_kv(
            &mut rows,
            None,
            "n / p / c / Shift+G",
            "Toggle NPC / PC / corpse / ground markers",
        );
        push_kv(
            &mut rows,
            None,
            "t / r / u",
            "Toggle pet / named / untargetable markers",
        );
        push_kv(
            &mut rows,
            None,
            "Alt+1-6",
            "Toggle geometry, spawns, paths, mesh, labels, annotations from anywhere on Map",
        );
        push_kv(
            &mut rows,
            None,
            "Alt+N / P / C / G / T / R / U",
            "Toggle the same entity filters when another tactical panel has focus",
        );
        rows.push(help_row(None, HelpCell::Text(String::new())));
        push_heading(&mut rows, None, "Spawn List");
        push_kv(&mut rows, None, "/", "Search spawns by name");
        push_kv(&mut rows, None, "f", "Cycle filter: All / PC / NPC / Named");
        push_kv(&mut rows, None, "j/k", "Navigate spawn list");
        push_kv(&mut rows, None, "t", "Navigate to selected spawn");
        push_kv(&mut rows, None, "a", "Target selected spawn (/target)");
        rows.push(help_row(None, HelpCell::Text(String::new())));
    }

    // Navigation screen
    {
        let marker = if app.active_screen == ActiveScreen::Navigation {
            Some(HelpFocus::Command("navigation"))
        } else {
            None
        };
        push_heading(&mut rows, marker, "Navigation — Pathfinding Controls");
        push_kv(
            &mut rows,
            None,
            "j/k or Up/Down",
            "Navigate client nav statuses",
        );
        push_kv(&mut rows, None, "Enter", "Jump back to the map panel");
        push_kv(&mut rows, None, ":nav <dest>", "Send a navigation command");
        push_kv(
            &mut rows,
            None,
            ":nav reload",
            "Refresh the active zone navmesh",
        );
        rows.push(help_row(None, HelpCell::Text(String::new())));
    }

    // Debug screen
    {
        let marker = if app.active_screen == ActiveScreen::Debug {
            Some(HelpFocus::Command("debug"))
        } else {
            None
        };
        push_heading(&mut rows, marker, "Debug — Oracle Controls");
        push_kv(&mut rows, None, "j/k", "Navigate focused debug panel");
        push_kv(&mut rows, None, "Enter", "Load selected offset or explorer row into Hex");
        push_kv(&mut rows, None, "h/x", "Open Hex memory for the selected spawn");
        push_kv(&mut rows, None, "/", "Search offsets or explorer rows");
        push_kv(&mut rows, None, "a", "Toggle hex dump annotations");
        push_kv(&mut rows, None, "c", "Cycle EQ Internals category filter");
        push_kv(&mut rows, None, "Tab", "Cycle Spawns / Hex / Explorer / Internals");
        rows.push(help_row(None, HelpCell::Text(String::new())));
    }

    // PacketMonitor screen
    {
        let marker = if app.active_screen == ActiveScreen::PacketMonitor {
            Some(HelpFocus::Command("packet_monitor"))
        } else {
            None
        };
        push_heading(&mut rows, marker, "PacketMonitor — Aethergram Controls");
        push_kv(&mut rows, None, "Space", "Pause / resume packet capture");
        push_kv(&mut rows, None, "j/k or ↑/↓", "Move the packet selection");
        push_kv(&mut rows, None, "PgUp/PgDn", "Scroll packet log");
        push_kv(&mut rows, None, "c", "Clear captured packets");
        rows.push(help_row(None, HelpCell::Text(String::new())));
    }

    // Economy screen
    {
        let marker = if app.active_screen == ActiveScreen::Economy {
            Some(HelpFocus::Command("economy"))
        } else {
            None
        };
        push_heading(&mut rows, marker, "Economy — Coinmark Controls");
        push_kv(&mut rows, None, "P", "Pause vendor/bank cycle");
        push_kv(&mut rows, None, "R", "Resume vendor/bank cycle");
        push_kv(&mut rows, None, "A", "Abort current cycle");
        push_kv(&mut rows, None, "S", "Skip current cycle");
        rows.push(help_row(None, HelpCell::Text(String::new())));
    }

    // Orchestrator screen
    {
        let marker = if app.active_screen == ActiveScreen::Orchestrator {
            Some(HelpFocus::Command("orchestrator"))
        } else {
            None
        };
        push_heading(&mut rows, marker, "Orchestrator — Third Gate Controls");
        rows.push(help_row(None, HelpCell::Text(String::new())));
    }

    // Metrics screen
    {
        let marker = if app.active_screen == ActiveScreen::Metrics {
            Some(HelpFocus::Command("metrics"))
        } else {
            None
        };
        push_heading(&mut rows, marker, "Metrics — Dashboard Controls");
        push_kv(&mut rows, None, "Tab", "Switch metric view forward");
        push_kv(&mut rows, None, "Shift+Tab", "Switch metric view backward");
        push_kv(&mut rows, None, "Up/Down", "Scroll metric rows");
        push_kv(&mut rows, None, "Enter", "Open or close metric detail");
        push_kv(&mut rows, None, "S", "Toggle metric value sort direction");
        push_kv(
            &mut rows,
            None,
            "Space",
            "Select or hide the current metric",
        );
        push_kv(
            &mut rows,
            None,
            "F",
            "Toggle fleet and selected-character metrics",
        );
        rows.push(help_row(None, HelpCell::Text(String::new())));
    }
    rows.push(help_row(None, HelpCell::Text(String::new())));

    push_heading(&mut rows, None, "Global Controls");
    push_kv(
        &mut rows,
        None,
        "1-8",
        "Switch screen: Soul Tethers / Cartography / Waypath / Oracle / Aethergram / Coinmark / Third Gate / Metrics",
    );
    push_kv(&mut rows, None, "[ ]", "Previous / next client");
    push_kv(&mut rows, None, "Tab", "Cycle panel focus");
    push_kv(&mut rows, None, ":", "Enter command mode");
    push_kv(&mut rows, None, "T", "Cycle theme");
    push_kv(&mut rows, None, "Esc", "Clear focus or filter");
    push_kv(&mut rows, None, "q", "Quit");
    rows.push(help_row(None, HelpCell::Text(String::new())));
    push_kv(
        &mut rows,
        Some(HelpFocus::Section(HelpSection::Workflows)),
        ":commands",
        "Full command reference (type in command mode)",
    );
    rows.push(help_row(None, HelpCell::Text(String::new())));
    rows.push(help_row(
        None,
        HelpCell::Text(String::from("Close: ? or Esc")),
    ));

    rows
}

fn render_help_outline(rows: Vec<HelpRow>, compact_rows: bool, app: &App) -> Vec<HelpLine> {
    let key_s = app.theme.help_key;
    let desc_s = app.theme.help_desc;
    let head_s = app.theme.help_heading;
    let dim_s = app.theme.help_dim;

    rows.into_iter()
        .map(|row| {
            let line = match row.cell {
                HelpCell::Heading(title) => Line::from(Span::styled(title, head_s)),
                HelpCell::KeyValue(key, value) => {
                    if compact_rows {
                        Line::from(vec![
                            Span::styled(format!(" {key} - "), key_s),
                            Span::styled(value, desc_s),
                        ])
                    } else {
                        Line::from(vec![
                            Span::styled(format!(" {key:<24}"), key_s),
                            Span::styled(value, desc_s),
                        ])
                    }
                }
                HelpCell::Text(text) => {
                    if text.is_empty() {
                        Line::from("")
                    } else {
                        Line::from(Span::styled(text, dim_s))
                    }
                }
            };
            HelpLine {
                marker: row.marker,
                line,
            }
        })
        .collect()
}

pub fn help_scroll_for_focus(app: &App, focus: HelpFocus) -> Option<usize> {
    build_help_outline(app)
        .iter()
        .position(|row| row.marker == Some(focus))
}

fn draw_help_overlay(frame: &mut Frame, area: Rect, app: &mut App) {
    let t = &app.theme;
    let popup_area = centered_popup(area, 92, 72, 96, 36, 96, 50, 1); // Fixed 96-wide
    let compact_rows = popup_area.width < 64;
    let rendered_rows = render_help_outline(build_help_outline(app), compact_rows, app);

    if let Some(focus) = app.help_focus.take()
        && let Some(index) = rendered_rows
            .iter()
            .position(|row| row.marker == Some(focus))
    {
        app.help_scroll = index.saturating_sub(1);
    }

    frame.render_widget(Clear, popup_area);

    let text: Vec<Line<'static>> = rendered_rows.into_iter().map(|row| row.line).collect();
    let visible_lines = popup_area.height.saturating_sub(2) as usize;
    let max_scroll = text.len().saturating_sub(visible_lines);
    let scroll = app.help_scroll.min(max_scroll);

    let title = if max_scroll > 0 {
        format!(
            " Help · Keybinds [{}%] (press ? to close) ",
            (scroll * 100) / max_scroll.max(1)
        )
    } else {
        String::from(" Help · Keybinds (press ? to close) ")
    };

    frame.render_widget(
        Paragraph::new(text)
            .scroll((scroll as u16, 0))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(t.border_type)
                    .title(Span::styled(title, t.text_bright))
                    .border_style(Style::default().fg(t.text_accent)) // cyan border
                    .style(Style::default().bg(t.help_bg)),
            ),
        popup_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        eq::structs::{
            BuffSlot, CastDurationSource, CastState as EqCastState, EqClass, SpawnInfo, SpawnType,
            StandState,
        },
        tui::app::{ChChainStatus, ClientState, GroupDef, NavClientStatus},
    };
    use ratatui::{Terminal, backend::TestBackend};
    use textquest_common::nav::NavStatus;

    #[test]
    fn overview_render_narrow_keeps_roster_dense() {
        let rendered = render_app(sample_app(), 80, 24);

        assert!(rendered.contains("Ops Roster"));
        assert!(rendered.contains("Toon06"));
        assert!(rendered.contains("Character"));
        assert!(rendered.contains("Camp"));
    }

    #[test]
    fn overview_render_medium_uses_compact_tabs_and_idle_character_card() {
        let rendered = render_app(sample_app(), 110, 30);

        assert!(rendered.contains("Teth"));
        assert!(rendered.contains("Way"));
        assert!(rendered.contains("Ora"));
        assert!(rendered.contains("Toon08"));
        assert!(!rendered.contains("y:"));
    }

    #[test]
    fn overview_render_wide_keeps_full_tabs_and_session_panel() {
        let rendered = render_app(sample_app(), 150, 36);

        assert!(rendered.contains("Soul Tethers"));
        assert!(rendered.contains("Waypath"));
        assert!(rendered.contains("Session"));
        assert!(rendered.contains("Toon10"));
    }

    #[test]
    fn overview_screen_dispatches_to_roster_renderer() {
        let overview = render_app(sample_app(), 150, 36);

        let mut navigation_app = sample_app();
        navigation_app.set_active_screen(ActiveScreen::Navigation);
        navigation_app.selected_client = 2;
        navigation_app.sync_from_selected_client();
        let navigation = render_app(navigation_app, 150, 36);

        assert!(overview.contains("Ops Roster"));
        assert!(overview.contains("Session"));
        assert!(!overview.contains("Selected Route"));

        assert!(navigation.contains("Selected Route"));
        assert!(!navigation.contains("Ops Roster"));
    }

    #[test]
    fn navigation_render_surfaces_route_state_and_blockers() {
        let mut app = sample_app();
        app.set_active_screen(ActiveScreen::Navigation);
        app.selected_client = 2;
        app.sync_from_selected_client();

        let rendered = render_app(app, 130, 34);

        assert!(rendered.contains("Selected Route"));
        assert!(rendered.contains("Navmesh route"));
        assert!(rendered.contains("No cached navmesh"));
    }

    #[test]
    fn help_overlay_small_host_uses_compact_rows() {
        let mut app = sample_app();
        app.help_visible = true;

        let rendered = render_app(app, 80, 24);

        assert!(rendered.contains("Help"));
        assert!(rendered.contains("Active: Soul Tethers"));
        assert!(rendered.contains("Soul Tethers Controls"));
    }

    #[test]
    fn tactical_help_overlay_lists_map_hotkeys_and_view_presets() {
        let mut app = sample_app();
        app.set_active_screen(ActiveScreen::Tactical);
        app.active_panel = ActivePanel::TacticalMap;
        app.help_visible = true;

        let rendered = render_app(app, 132, 34);

        assert!(rendered.contains("Ctrl+L"));
        assert!(rendered.contains("Ctrl+G"));
        assert!(rendered.contains("Ctrl+A"));
        assert!(rendered.contains("Shift+I"));
        assert!(rendered.contains("End"));
        assert!(rendered.contains("Shift+G"));
        assert!(rendered.contains("Alt+N / P / C / G / T / R / U"));
    }

    #[test]
    fn help_scroll_focus_finds_command_reference_anchor() {
        let app = sample_app();
        let workflows_anchor =
            help_scroll_for_focus(&app, HelpFocus::Section(HelpSection::Workflows));

        // Workflows anchor exists (used by :commands navigation)
        assert!(workflows_anchor.is_some());
    }

    #[test]
    fn toast_overlay_truncates_cleanly_in_narrow_terminals() {
        let mut app = sample_app();
        let message = "Repeated camp status updates should stay readable in narrow terminals";
        app.set_toast(ToastLevel::Warning, message);

        let rendered = render_app(app, 38, 12);

        assert!(rendered.contains("WARN"));
        assert!(rendered.contains(".."));
        assert!(!rendered.contains(message));
    }

    #[test]
    fn toast_appears_at_bottom_not_overlapping_header() {
        let mut app = sample_app();
        let message = "Toast at bottom";
        app.set_toast(ToastLevel::Info, message);

        let rendered = render_app(app, 60, 15);
        let lines: Vec<&str> = rendered.split('\n').collect();

        // Toast should be visible
        assert!(
            rendered.contains("INFO"),
            "Toast message not found in rendered output"
        );

        // Check that toast appears in the bottom area (around row 11-13)
        // Terminal is 15 rows: header (rows 0-2), body (rows 3-10), status bar (rows
        // 11-13) Toast should appear at row 10 (just above status bar at row
        // 11)
        let has_toast_in_lower_area = lines
            .iter()
            .skip(8) // Start checking from row 8 onwards
            .any(|line| line.contains("INFO"));

        assert!(
            has_toast_in_lower_area,
            "Toast should appear in lower area, not at top"
        );
    }

    #[test]
    fn diagnostics_overlay_surfaces_codes_and_suggestions() {
        let mut app = sample_app();
        app.set_feedback(
            ToastLevel::Error,
            "Failed to send IPC command to PID 42: pipe closed",
            false,
        );
        app.diagnostics_visible = true;

        let rendered = render_app(app, 120, 32);

        assert!(rendered.contains("Diagnostics"));
        assert!(rendered.contains("TQ-TUI-E"));
        assert!(rendered.contains("Suggestion"));
        assert!(rendered.contains("IPC throughput"));
    }

    fn render_app(mut app: App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| draw(frame, &mut app))
            .expect("render app");
        buffer_contents(terminal.backend().buffer(), width, height)
    }

    fn buffer_contents(buf: &ratatui::buffer::Buffer, width: u16, height: u16) -> String {
        (0..height)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn sample_app() -> App {
        let mut app = App::new();
        app.groups = (0..6)
            .map(|idx| GroupDef {
                id: (idx + 1) as u8,
                name: format!("Group {}", idx + 1),
                account_range: ((idx * 6 + 1) as u8, ((idx + 1) * 6) as u8),
                default_camp: format!("Camp {}", idx + 1),
            })
            .collect();
        app.server_name = String::from("Fippy Darkpaw");
        app.status_message = String::from("Operator demo ready");
        app.main_assist = Some(String::from("Toon01"));
        app.main_tank = Some(String::from("Toon02"));
        app.ch_chain_status = Some(ChChainStatus {
            members: 4,
            interval_secs: 2.5,
            is_adaptive: true,
            target_id: 42,
        });
        app.clients = (1..=12).map(sample_client).collect();
        app.selected_client = 0;
        app.sync_from_selected_client();
        app.nav_state.nav_statuses.insert(
            app.clients[2].pid,
            NavClientStatus {
                destination: String::from("camp"),
                status: NavStatus::Moving {
                    waypoint_index: 0,
                    waypoint_count: 3,
                    distance_remaining: 15.0,
                },
                eta_secs: Some(12),
                waypoints: Vec::new(),
                path_exists: true,
                path_length: Some(42.0),
                failure_reason: None,
                route_state: String::from("Navmesh route"),
                recovery_state: None,
                blockers: vec![String::from(
                    "No cached navmesh for poknowledge; using straight-line fallback.",
                )],
                is_demo_scripted: false,
                blocker_description: None,
                retry_count: 0,
                fallback_route: None,
                progress_pct: 0.0,
            },
        );
        app
    }

    fn sample_client(index: u8) -> ClientState {
        let mut client = ClientState::new(1_000 + u32::from(index), 0);
        let name = format!("Toon{index:02}");
        client.character_name = name.clone();
        client.zone_name = if index <= 6 {
            String::from("Guild Lobby")
        } else {
            String::from("Plane of Knowledge")
        };
        client.local_player = Some(sample_spawn(
            &name,
            match index % 4 {
                0 => EqClass::Cleric,
                1 => EqClass::Warrior,
                2 => EqClass::Enchanter,
                _ => EqClass::Wizard,
            },
            65,
            if index == 2 {
                Some(EqCastState {
                    spell_id: 1,
                    spell_name: Some(String::from("Complete Heal")),
                    target_id: 42,
                    spell_eta: 0,
                    item_id: 0,
                    spell_slot: 0,
                    remaining_ms: Some(2_500),
                    total_cast_ms: Some(2_500),
                    duration_source: CastDurationSource::SpellDataBase,
                    gem_etas: None,
                })
            } else {
                None
            },
            StandState::Standing,
        ));
        if index == 1 {
            client.target = Some(sample_target("Ancient Cyclops", 42));
        }
        client
    }

    fn sample_spawn(
        name: &str,
        class: EqClass,
        level: u8,
        cast_state: Option<EqCastState>,
        stand_state: StandState,
    ) -> SpawnInfo {
        SpawnInfo {
            name: name.to_string(),
            displayed_name: name.to_string(),
            lastname: String::new(),
            spawn_id: 100,
            spawn_type: SpawnType::Player,
            level,
            class_id: class as u8,
            class: Some(class),
            stand_state,
            x: 100.0,
            y: -25.0,
            z: 5.0,
            heading: 0.0,
            hp_current: 9_500,
            hp_max: 10_000,
            mana_current: 7_000,
            mana_max: 8_000,
            endurance_current: 4_000,
            endurance_max: 5_000,
            is_gm: false,
            race_id: 1,
            buff_slots: vec![BuffSlot {
                spell_id: 0xFFFF,
                duration_ticks: 0,
                caster_level: 0,
            }],
            spellbook: Vec::new(),
            memorized_spells: Vec::new(),
            cast_state,
        }
    }

    fn sample_target(name: &str, spawn_id: u32) -> SpawnInfo {
        SpawnInfo {
            name: name.to_string(),
            displayed_name: name.to_string(),
            lastname: String::new(),
            spawn_id,
            spawn_type: SpawnType::Npc,
            level: 65,
            class_id: 1,
            class: Some(EqClass::Warrior),
            stand_state: StandState::Standing,
            x: 95.0,
            y: -30.0,
            z: 5.0,
            heading: 0.0,
            hp_current: 50_000,
            hp_max: 50_000,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            is_gm: false,
            race_id: 1,
            buff_slots: Vec::new(),
            spellbook: Vec::new(),
            memorized_spells: Vec::new(),
            cast_state: None,
        }
    }
}
