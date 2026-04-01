//! TUI renderer — entry point and global chrome (header, status bar, help overlay).
//!
//! Each screen lives in its own sub-module:
//! - [`dashboard`]   — character grid + health gauges + session stats
//! - [`spawns`]      — filterable spawn list + hex dump
//! - [`map`]         — zone map + named tracker
//! - [`groups`]      — per-group panels with buff timer columns
//! - [`navigation`]  — nav status + commands reference
//! - [`widgets`]     — shared helpers (`panel`, `themed_header_row`, colour fns …)

pub mod ch_chain;
pub mod dashboard;
pub mod groups;
pub mod map;
pub mod navigation;
pub mod spawns;
pub mod widgets;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::tui::app::{ActivePanel, ActiveScreen, App};
use crate::tui::ui::widgets::{
    WidthClass, centered_popup, classify_width, line_width, spans_width, truncate_inline,
};

// ─── Entry point ─────────────────────────────────────────────────────────────

/// Top-level render function — applies outer margin then dispatches to the active screen.
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
        ActiveScreen::Overview => dashboard::draw_dashboard(frame, outer[1], app),
        ActiveScreen::Tactical => map::draw_map_screen(frame, outer[1], app),
        ActiveScreen::Navigation => navigation::draw_navigation_screen(frame, outer[1], app),
        ActiveScreen::Debug => spawns::draw_debug_screen(frame, outer[1], app),
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

    // Config panel overlay
    if app.config_panel_state.active {
        use crate::tui::config_panel::ConfigPanelWidget;
        let popup_area = centered_popup(area, 68, 72, 36, 12, 96, 30, 1);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            ConfigPanelWidget::new(&app.config_panel_state).accent_color(app.theme.text_accent),
            popup_area,
        );
    }

    // CH chain panel overlay
    if app.ch_chain_panel_state.active {
        use crate::tui::ui::ch_chain::ChChainWidget;
        let popup_area = centered_popup(area, 80, 78, 48, 12, 104, 32, 1);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            ChChainWidget::new(&app.ch_chain_panel_state).accent_color(app.theme.text_accent),
            popup_area,
        );
    }

    // Wizard overlay
    if app.wizard_state.active {
        use crate::tui::wizard::WizardWidget;
        frame.render_widget(Clear, frame.area());
        frame.render_widget(
            WizardWidget::new(&app.wizard_state).accent_color(app.theme.text_accent),
            frame.area(),
        );
    }

    // Toast notification
    if let Some(ref toast) = app.toast_message {
        let toast_text = truncate_inline(toast, area.width.saturating_sub(4) as usize);
        let toast_width = (toast_text.chars().count() + 2).min(area.width as usize) as u16;
        let toast_x = area.x + area.width.saturating_sub(toast_width).saturating_sub(1);
        let toast_y = area.y + 1;
        let toast_area = Rect::new(toast_x, toast_y, toast_width, 1);
        frame.render_widget(Clear, toast_area);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {toast_text} "),
                Style::default().fg(Color::Black).bg(app.theme.text_accent),
            ))),
            toast_area,
        );
    }
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
            ActiveScreen::Overview => "Char",
            ActiveScreen::Tactical => "Map",
            ActiveScreen::Navigation => "Nav",
            ActiveScreen::Debug => "Dbg",
        },
        WidthClass::Narrow => match screen {
            ActiveScreen::Overview => "1",
            ActiveScreen::Tactical => "2",
            ActiveScreen::Navigation => "3",
            ActiveScreen::Debug => "4",
        },
    }
}

fn build_header_tabs(app: &App, width_class: WidthClass) -> Line<'static> {
    let t = &app.theme;
    let mut spans = Vec::new();
    for (index, screen) in ActiveScreen::ALL.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        let label = header_tab_label(*screen, width_class);
        spans.push(if *screen == app.active_screen {
            Span::styled(format!(" {label} "), t.tab_active)
        } else {
            Span::styled(format!(" {label} "), t.tab_inactive)
        });
    }
    Line::from(spans)
}

fn build_header_meta(app: &App, width_class: WidthClass, max_width: usize) -> Vec<Span<'static>> {
    let t = &app.theme;
    let client_count = app.clients.len();
    let client_label = if client_count > 0 {
        match width_class {
            WidthClass::Narrow => format!("{client_count} EQ"),
            _ => format!("{client_count}x EQ"),
        }
    } else {
        String::from("Not attached")
    };
    let selected_name_budget = match width_class {
        WidthClass::Narrow => 10,
        WidthClass::Medium => 16,
        WidthClass::Wide => 24,
    };
    let selected_label = if let Some(client) = app.active_client() {
        let name = client.local_player.as_ref().map_or_else(
            || String::from("???"),
            |player| app.redact_name(&player.displayed_name).into_owned(),
        );
        format!(
            "{}/{} {}",
            app.selected_client + 1,
            client_count.max(1),
            truncate_inline(&name, selected_name_budget)
        )
    } else {
        String::from("No client")
    };
    let group_budget = match width_class {
        WidthClass::Narrow => 10,
        WidthClass::Medium => 14,
        WidthClass::Wide => 18,
    };
    let server_budget = match width_class {
        WidthClass::Narrow => 8,
        WidthClass::Medium => 12,
        WidthClass::Wide => 18,
    };
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
    let group_style = if app.active_group.is_some() {
        t.header_group_active.add_modifier(Modifier::BOLD)
    } else {
        t.header_group
    };
    let separator = Style::default().fg(t.border_dim.fg.unwrap_or(Color::DarkGray));
    let mut spans = Vec::new();
    let _ = push_segment_if_fits(
        &mut spans,
        vec![Span::styled(client_label, t.header_client_count)],
        max_width,
    );
    let _ = push_segment_if_fits(
        &mut spans,
        vec![
            Span::styled(" | ", separator),
            Span::styled(selected_label, t.header_selected),
        ],
        max_width,
    );
    let _ = push_segment_if_fits(
        &mut spans,
        vec![
            Span::styled(" | ", separator),
            Span::styled(
                truncate_inline(&app.group_focus_label(), group_budget),
                group_style,
            ),
        ],
        max_width,
    );
    let _ = push_segment_if_fits(
        &mut spans,
        vec![
            Span::styled(" | ", separator),
            Span::styled(
                truncate_inline(app.display_server(), server_budget),
                Style::default().fg(t.text_server),
            ),
        ],
        max_width,
    );
    let _ = push_segment_if_fits(
        &mut spans,
        vec![
            Span::styled(" | ", separator),
            Span::styled(truncate_inline(&zone_label, zone_budget), t.header_zone),
        ],
        max_width,
    );
    spans
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let block = widgets::panel(" DMFT ", t.border_dim, t);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let width_class = classify_width(inner.width);
    let tabs = build_header_tabs(app, width_class);
    let tabs_width = line_width(&tabs).min(inner.width as usize) as u16;

    if inner.width > tabs_width.saturating_add(12) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(10), Constraint::Length(tabs_width)])
            .split(inner);
        let header_spans = build_header_meta(app, width_class, cols[0].width as usize);
        frame.render_widget(Paragraph::new(Line::from(header_spans)), cols[0]);
        frame.render_widget(Paragraph::new(tabs).alignment(Alignment::Right), cols[1]);
    } else {
        let mut header_spans = build_header_meta(app, width_class, inner.width as usize);
        let _ = push_segment_if_fits(
            &mut header_spans,
            vec![
                Span::styled(
                    " | ",
                    Style::default().fg(t.border_dim.fg.unwrap_or(Color::DarkGray)),
                ),
                Span::styled(
                    header_tab_label(app.active_screen, width_class),
                    t.tab_active,
                ),
            ],
            inner.width as usize,
        );
        frame.render_widget(Paragraph::new(Line::from(header_spans)), inner);
    }
}

// ─── Status bar ──────────────────────────────────────────────────────────────

fn status_hints(app: &App, width_class: WidthClass) -> &'static [(&'static str, &'static str)] {
    if app.active_panel == ActivePanel::TacticalMap {
        match width_class {
            WidthClass::Narrow => &[
                ("1-4", "screen"),
                ("Tab", "pane"),
                ("[ ]", "client"),
                ("n", "mesh"),
                ("v", "view"),
                ("+/-", "depth"),
                ("?", "help"),
            ],
            WidthClass::Medium => &[
                ("1-4", "screen"),
                ("Tab", "pane"),
                ("[ ]", "client"),
                ("Alt+1", "geo"),
                ("Alt+2", "spawns"),
                ("Alt+3", "paths"),
                ("Alt+4", "mesh"),
                ("n", "navmesh"),
                ("v", "view"),
                ("PgUp/Dn", "zoom"),
                ("?", "help"),
            ],
            WidthClass::Wide => &[
                ("1-4", "screen"),
                ("Tab", "pane"),
                ("[ ]", "client"),
                ("Alt+1", "geo"),
                ("Alt+2", "spawns"),
                ("Alt+3", "paths"),
                ("Alt+4", "mesh"),
                ("n", "navmesh"),
                ("Alt+5", "labels"),
                ("v", "view"),
                ("PgUp/Dn", "zoom"),
                ("+/-", "depth"),
                ("Home", "reset"),
                ("?", "help"),
            ],
        }
    } else {
        match width_class {
            WidthClass::Narrow => &[
                ("1-4", "screen"),
                ("Tab", "pane"),
                ("[ ]", "client"),
                ("/", "search"),
                ("g/v", "sect"),
                ("?", "help"),
            ],
            WidthClass::Medium => &[
                ("1-4", "screen"),
                ("Shift+1-6", "group"),
                ("Tab", "pane"),
                ("[ ]", "client"),
                ("g/v", "sections"),
                ("z", "collapse"),
                ("/", "search"),
                ("f", "filter"),
                ("?", "help"),
            ],
            WidthClass::Wide => &[
                ("1-4", "screen"),
                ("Shift+1-6", "group"),
                ("Tab", "pane"),
                ("[ ]", "client"),
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
    let message_cap = match width_class {
        WidthClass::Narrow => max_width.saturating_sub(1),
        WidthClass::Medium => max_width.min(38),
        WidthClass::Wide => max_width.min(48),
    };
    let mut spans = vec![
        Span::raw(" "),
        Span::styled(
            truncate_inline(&app.status_message, message_cap.saturating_sub(1)),
            t.statusbar_message,
        ),
    ];
    for (key, desc) in status_hints(app, width_class) {
        let segment = if spans.len() == 2 {
            let mut items = vec![Span::styled(" | ", t.statusbar_dim)];
            items.extend(widgets::keybinding_hint(key, desc, t));
            items
        } else {
            widgets::keybinding_hint(key, desc, t)
        };
        if !push_segment_if_fits(&mut spans, segment, max_width) {
            break;
        }
    }
    spans
}

fn build_status_right(app: &App, width_class: WidthClass, max_width: usize) -> Vec<Span<'static>> {
    let t = &app.theme;
    let mode_str = format!("{}", app.operating_mode);
    let mode_bg = match mode_str.as_str() {
        "Camp" => t.mode_camp,
        "Hunt" => t.mode_hunt,
        _ => t.text_muted,
    };
    let mut spans = vec![Span::styled(
        format!(" {mode_str} "),
        Style::default()
            .fg(Color::Black)
            .bg(mode_bg)
            .add_modifier(Modifier::BOLD),
    )];
    let filter = app.spawns_state.spawn_type_filter.label();
    if filter != "All" {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::raw(" "),
                Span::styled(
                    format!(" {filter} "),
                    Style::default()
                        .fg(Color::Black)
                        .bg(t.text_accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ],
            max_width,
        );
    }
    if app.privacy_mode {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![Span::raw(" "), Span::styled(" PRIVATE ", t.statusbar_badge)],
            max_width,
        );
    }
    if let Some(idx) = app.active_group {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::raw(" "),
                Span::styled(
                    format!(" G{} ", idx + 1),
                    Style::default().fg(Color::Black).bg(t.text_accent),
                ),
            ],
            max_width,
        );
    }
    if width_class != WidthClass::Narrow {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::raw(" "),
                Span::styled(format!(" {} ", app.theme_kind.label()), t.statusbar_dim),
            ],
            max_width,
        );
    }
    spans
}

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    // Command mode: full-width input line with syntax hint
    if app.cmd_state.command_mode {
        let input_text = format!(": {}_", app.cmd_state.command_buffer);
        let mut spans = vec![Span::styled(&input_text, t.statusbar_cmd)];

        // Show syntax hint for known commands
        if let Some(hint) = crate::tui::app::command_syntax_hint(&app.cmd_state.command_buffer) {
            let hint_budget = area
                .width
                .saturating_sub(input_text.chars().count() as u16)
                .saturating_sub(8) as usize;
            spans.push(Span::styled(
                format!("  ({})", truncate_inline(hint, hint_budget)),
                Style::default().fg(t.text_muted),
            ));
        }

        frame.render_widget(
            Paragraph::new(Line::from(spans)).block(widgets::panel("", t.border_active, t)),
            area,
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
        .split(area);
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

// ─── Help overlay ─────────────────────────────────────────────────────────────

fn draw_help_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let popup_area = centered_popup(area, 76, 80, 42, 14, 88, 40, 1);
    let compact_rows = popup_area.width < 64;

    frame.render_widget(Clear, popup_area);

    let key_s = t.help_key;
    let desc_s = t.help_desc;
    let head_s = t.help_heading;
    let dim_s = t.help_dim;

    let kv = |k: &'static str, v: &'static str| -> Line<'static> {
        if compact_rows {
            Line::from(vec![
                Span::styled(format!(" {k} - "), key_s),
                Span::styled(v, desc_s),
            ])
        } else {
            Line::from(vec![
                Span::styled(format!(" {k:<14}"), key_s),
                Span::styled(v, desc_s),
            ])
        }
    };

    // Build context-sensitive quick-reference for the active screen
    let mut text: Vec<Line<'_>> = Vec::with_capacity(160);

    let screen_label = app.active_screen.label();
    text.push(Line::from(vec![
        Span::styled(
            format!(" Active: {screen_label} "),
            Style::default()
                .fg(t.help_bg)
                .bg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if compact_rows {
                " - keys below"
            } else {
                " -- keys for this screen shown below"
            },
            dim_s,
        ),
    ]));
    text.push(Line::from(""));

    match app.active_screen {
        ActiveScreen::Overview => {
            text.push(kv("g", "Toggle group roster section"));
            text.push(kv("v", "Toggle scope/filters section"));
            text.push(kv("z", "Collapse or expand focused section"));
            text.push(kv("j/k / Up/Dn", "Navigate client roster"));
            text.push(kv("Enter", "Expand selected character detail"));
            text.push(kv("e / d / l", "Engage / Disengage / Loot (quick keys)"));
            text.push(kv("r", "Repeat last command"));
        }
        ActiveScreen::Tactical => {
            text.push(kv("+/-", "Adjust Z-depth slice filter"));
            text.push(kv("Arrows", "Pan map viewport"));
            text.push(kv("PgUp/PgDn", "Zoom map in/out"));
            text.push(kv("Home", "Reset map viewport"));
            text.push(kv("m / M", "Toggle map maximize (full screen)"));
            text.push(kv("v (map)", "Cycle viewport: auto/local/global"));
            text.push(kv("n (map)", "Toggle navmesh overlay on map"));
            text.push(kv("Enter (map)", "Toggle map maximize"));
            text.push(kv("/", "Search spawns in spawn list"));
            text.push(kv("f", "Cycle spawn type filter"));
            text.push(kv("j/k", "Navigate spawn list"));
            text.push(kv("Enter (list)", "Inspect selected spawn"));
        }
        ActiveScreen::Navigation => {
            text.push(kv("j/k / Up/Dn", "Navigate client list"));
            text.push(kv("Enter", "Toggle full nav status view"));
            text.push(kv("e / d", "Engage / Disengage (quick keys)"));
            text.push(kv(":nav <dest>", "Send navigation command"));
        }
        ActiveScreen::Debug => {
            text.push(kv("Up/Down", "Scroll hex dump view"));
            text.push(kv("j/k", "Navigate spawn list"));
            text.push(kv("PgUp/PgDn", "Page through spawn list"));
            text.push(kv("Home / End", "Jump to first/last spawn"));
            text.push(kv("Enter", "Inspect spawn in hex view"));
            text.push(kv("/", "Search spawns by name"));
            text.push(kv("f", "Cycle spawn type filter"));
        }
    }
    text.push(Line::from(""));

    // ── Full reference follows ──
    text.extend_from_slice(&[
        // ── Global Keybindings ──
        Line::from(Span::styled(" Global Keybindings", head_s)),
        Line::from(""),
        kv("1-4", "Switch screen: Characters, Map, Navigation, Debug"),
        kv("Shift+1-6", "Focus group G1-G6"),
        kv("Shift+0", "Show all groups (clear group filter)"),
        kv("Tab", "Cycle focused pane within current screen"),
        kv("[ ]", "Cycle through connected clients"),
        kv(":", "Enter command mode"),
        kv("/", "Search spawns (switches to Map screen)"),
        kv("f", "Cycle spawn type filter (All/PC/NPC/Named)"),
        kv("z", "Collapse or expand focused section"),
        kv("p", "Toggle privacy mode (redact names)"),
        kv("T", "Cycle color theme"),
        kv("?", "Toggle this help overlay"),
        kv("q / Ctrl+C", "Quit application"),
        kv("Esc", "Clear group filter or close overlay"),
        Line::from(""),
        // ── Quick Action Keys ──
        Line::from(Span::styled(" Quick Action Keys (any screen)", head_s)),
        Line::from(""),
        kv("e", "Engage combat (send to focused clients)"),
        kv("d", "Disengage combat (send to focused clients)"),
        kv("l", "Loot nearby corpses"),
        kv("r", "Repeat last command from history"),
        kv("F1-F9", "Run favorite command (by frequency)"),
        Line::from(""),
        // ── Characters Screen (1) ──
        Line::from(Span::styled(" Characters Screen [1]", head_s)),
        Line::from(""),
        kv("g", "Toggle group roster section"),
        kv("v", "Toggle scope/filters section"),
        kv("j/k / Up/Dn", "Navigate client roster"),
        kv("Enter", "Expand selected character detail"),
        Line::from(""),
        // ── Map Screen (2) ──
        Line::from(Span::styled(" Map Screen [2]", head_s)),
        Line::from(""),
        kv("+/-", "Adjust Z-depth slice filter"),
        kv("Arrow keys", "Pan map viewport"),
        kv("PgUp/PgDn", "Zoom map in/out"),
        kv("Home", "Reset map viewport to default"),
        kv("m / M", "Toggle map maximize (full screen)"),
        kv("v (map pane)", "Cycle viewport: auto/local/global"),
        kv("n (map pane)", "Toggle navmesh overlay"),
        kv("Enter (map)", "Toggle map maximize"),
        Line::from(""),
        // ── Map Spawn List ──
        Line::from(Span::styled(" Spawn List (Map/Debug)", head_s)),
        Line::from(""),
        kv("j/k / Up/Dn", "Navigate spawn list"),
        kv("PgUp/PgDn", "Page through spawn list"),
        kv("Home / End", "Jump to first/last spawn"),
        kv("Enter", "Inspect selected spawn in hex dump"),
        Line::from(""),
        // ── Navigation Screen (3) ──
        Line::from(Span::styled(" Navigation Screen [3]", head_s)),
        Line::from(""),
        kv("j/k / Up/Dn", "Navigate client list"),
        kv("Enter", "Toggle full navigation status view"),
        Line::from(""),
        // ── Debug Screen (4) ──
        Line::from(Span::styled(" Debug Screen [4]", head_s)),
        Line::from(""),
        kv("Up/Down", "Scroll hex dump"),
        Line::from(""),
        // ── Command Mode ──
        Line::from(Span::styled(" Command Mode (: prefix)", head_s)),
        Line::from(""),
        kv("Tab", "Auto-complete command or arguments"),
        kv("Up/Down", "Browse command history"),
        kv("Enter", "Execute command"),
        kv("Esc", "Cancel and exit command mode"),
        Line::from(""),
        // ── Panel Focus Guide ──
        Line::from(Span::styled(" Panel Focus Guide (Tab to cycle)", head_s)),
        Line::from(""),
        kv(
            "[1] Overview",
            "Roster, Character, Groups, Filters, Combat, Session",
        ),
        kv("[2] Map", "Map, Spawns, Named, Navigation"),
        kv("[3] Navigation", "Client list, Navigation status"),
        kv("[4] Debug", "Spawn list, Hex dump"),
        Line::from(""),
        // ── Search & Filter ──
        Line::from(Span::styled(" Search & Filter", head_s)),
        Line::from(""),
        kv("/", "Open spawn search (text filter, live update)"),
        kv("Esc / Enter", "Close search (keeps filter active)"),
        kv("f", "Cycle type filter: All -> PC -> NPC -> Named"),
        kv("Esc (no srch)", "Clear active spawn filter"),
        Line::from(""),
        // ── Targeting Commands ──
        Line::from(Span::styled(" Targeting Commands", head_s)),
        Line::from(""),
        kv("<name> /cmd", "Send slash command to character by name"),
        kv("@<name> /cmd", "Force direct target (bypass group)"),
        kv("<pid> /cmd", "Send slash command to client by PID"),
        kv("G1-G6 /cmd", "Send slash command to entire group"),
        kv("all /cmd", "Broadcast slash command to all clients"),
        Line::from(""),
        // ── Common Slash Commands ──
        Line::from(Span::styled(
            " Common Slash Commands (via targeting)",
            head_s,
        )),
        Line::from(""),
        kv("/sit", "Sit down (meditate for mana)"),
        kv("/stand", "Stand up"),
        kv("/camp", "Camp out (log out to char select)"),
        kv("/follow <n>", "Auto-follow target"),
        kv("/assist <n>", "Assist target (match their target)"),
        kv("/disband", "Leave current group"),
        kv("/target <n>", "Target a specific mob or player"),
        kv("/cast <slot>", "Cast spell from gem slot number"),
        Line::from(""),
        // ── General Commands ──
        Line::from(Span::styled(" General Commands", head_s)),
        Line::from(""),
        kv("help", "Show this help overlay"),
        kv("commands", "List all commands with usage summary"),
        kv("status", "Show connected client count"),
        kv("mode <m>", "Switch mode: camp | hunt"),
        kv("inject", "Request DLL injection (placeholder)"),
        Line::from(""),
        // ── Combat Commands ──
        Line::from(Span::styled(" Combat Commands", head_s)),
        Line::from(""),
        kv("ma [name]", "Set or show Main Assist (sends /assist)"),
        kv("mt [name]", "Set or show Main Tank"),
        kv("engage [id]", "Start combat (optional target_id)"),
        kv("disengage", "Stop combat for focused clients"),
        kv("loot", "Loot nearby corpses"),
        kv("heal cancel", "Toggle heal-cancel optimization"),
        Line::from(""),
        // ── Group & Social Commands ──
        Line::from(Span::styled(" Group & Social Commands", head_s)),
        Line::from(""),
        kv("invite <name>", "Send group invite from active client"),
        kv("accept", "Accept pending group invite"),
        Line::from(""),
        // ── Navigation Commands ──
        Line::from(Span::styled(" Navigation Commands", head_s)),
        Line::from(""),
        kv("nav <dest>", "Navigate to camp, coords (x y z), or zone"),
        kv("track <name>", "Track a spawn by name (shows on map)"),
        kv("track list", "Show all tracked spawns"),
        kv("untrack <name>", "Stop tracking a spawn"),
        Line::from(""),
        // ── Camp Commands ──
        Line::from(Span::styled(" Camp Commands", head_s)),
        Line::from(""),
        kv("camp start <n>", "Start camp from config/camps/<n>.toml"),
        kv("camp stop", "Stop active camp"),
        kv("camp status", "Show active camp status"),
        kv("camp list", "List saved camp configs"),
        kv("camp add <n>", "Save current position as camp <n>"),
        kv("camp remove <n>", "Delete saved camp config"),
        kv("camp next", "Advance to next linked camp"),
        kv("camp prev", "Fall back to previous linked camp"),
        kv("camp <name>", "Shortcut for camp start <name>"),
        Line::from(""),
        // ── Login Commands ──
        Line::from(Span::styled(" Login & Lifecycle Commands", head_s)),
        Line::from(""),
        kv("login", "List configured accounts and status"),
        kv("login all", "Launch all configured accounts"),
        kv("login G<n>", "Launch accounts in group n"),
        kv("login <name>", "Launch a single account by name"),
        kv("stop <name>", "Stop a client (or stop all)"),
        kv("restart <name>", "Restart a client (or restart all)"),
        Line::from(""),
        // ── CH Chain Commands ──
        Line::from(Span::styled(" CH Chain Commands", head_s)),
        Line::from(""),
        kv("ch status", "Show CH chain status"),
        kv("ch start", "<pids> <interval> <target> [slot]"),
        kv("ch stop", "Stop the running CH chain"),
        kv("ch add <pid>", "Add cleric to chain"),
        kv("ch rm <pid>", "Remove cleric from chain"),
        kv("ch interval", "Set cast interval (seconds)"),
        kv("ch adaptive", "Toggle adaptive timing: on|off"),
        Line::from(""),
        // ── Status Glyphs ──
        Line::from(Span::styled(" Status Glyphs", head_s)),
        Line::from(""),
        kv("⚔ / ✚ / ✦", "Fight, Heal, Cast"),
        kv("➜ / ✓ / !", "Navigate, Arrived, Stuck"),
        kv("☾ / ⇣ / ⌕", "Sit, Feign, Loot"),
        Line::from(""),
        // ── Usage Examples ──
        Line::from(Span::styled(" Usage Examples", head_s)),
        Line::from(""),
        kv("Cleric /sit", "Send /sit to character named Cleric"),
        kv("G1 /follow MA", "All of group 1 follow the Main Assist"),
        kv("all /camp", "Tell every client to camp out"),
        kv("nav gfay", "Navigate to Greater Faydark"),
        kv("nav 100 -50 5", "Navigate to coordinates x=100 y=-50 z=5"),
        kv("camp start orc", "Start camp from config/camps/orc.toml"),
        kv("ma Warrior", "Set Warrior as Main Assist"),
        kv("ch start 1,2", "Start CH chain with PIDs 1,2"),
        kv("track Fippy", "Track spawn named Fippy on the map"),
        kv("login all", "Launch all configured accounts"),
        kv("mode hunt", "Switch to hunt mode (roaming pulls)"),
        Line::from(""),
        // ── Tips ──
        Line::from(Span::styled(" Tips", head_s)),
        Line::from(""),
        kv("Tab", "Auto-complete in command mode (context-aware)"),
        kv("Up/Down", "Browse command history in command mode"),
        kv("F1-F9", "Quick-launch your most-used commands"),
        kv("Shift+digit", "Group focus filters all panels to G1-G6"),
        kv(":commands", "Show all commands in the status bar"),
        Line::from(""),
        // ── Common Errors ──
        Line::from(Span::styled(" Common Errors & Solutions", head_s)),
        Line::from(""),
        kv("No client", "Select a client with [ ] before sending cmds"),
        kv("Unknown cmd", "Check spelling; type :commands for list"),
        kv("Did you mean", "Typo detected; suggestion shown in status"),
        kv("No MA set", "Use :ma <name> to set Main Assist first"),
        kv("No camp cfg", "Create config/camps/<name>.toml first"),
        kv("No accounts", "Create config/accounts.toml for :login"),
        kv("Pipe error", "Client may have crashed; check :status"),
        Line::from(""),
        // ── Configuration Files ──
        Line::from(Span::styled(" Configuration Files", head_s)),
        Line::from(""),
        kv("dmft", "config/frostreaver.toml (main config)"),
        kv("accounts", "config/accounts.toml (login accounts)"),
        kv("camps", "config/camps/<name>.toml (camp positions)"),
        Line::from(""),
        // ── Operating Modes ──
        Line::from(Span::styled(" Operating Modes", head_s)),
        Line::from(""),
        kv("Camp mode", "Hold position, pull mobs to camp center"),
        kv("Hunt mode", "Roam and pull, follow waypoint paths"),
        kv(":mode camp", "Switch to camp mode"),
        kv(":mode hunt", "Switch to hunt mode"),
        Line::from(""),
        // ── Group Targeting ──
        Line::from(Span::styled(" Group Targeting Reference", head_s)),
        Line::from(""),
        kv("G1-G6", "Send command to specific group (1-6)"),
        kv("all", "Send command to every connected client"),
        kv("<name>", "Send command to specific character"),
        kv("@<name>", "Force direct (bypass group filtering)"),
        kv("<pid>", "Send command to client by process ID"),
        Line::from(""),
        Line::from(Span::styled(
            " Scroll: j/k/Up/Down  Page: PgUp/PgDn  Top: Home  Close: ?/Esc",
            dim_s,
        )),
    ]);

    // Clamp scroll to valid range (account for border lines)
    let visible_lines = popup_area.height.saturating_sub(2) as usize;
    let max_scroll = text.len().saturating_sub(visible_lines);
    let scroll = app.help_scroll.min(max_scroll);

    // Build title with scroll indicator
    let title = if max_scroll > 0 {
        let pct = if max_scroll > 0 {
            (scroll * 100) / max_scroll
        } else {
            0
        };
        format!(" Help [{pct}%] ")
    } else {
        String::from(" Help ")
    };

    frame.render_widget(
        Paragraph::new(text)
            .scroll((scroll as u16, 0))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(t.border_type)
                    .title(Span::styled(title, t.help_heading))
                    .border_style(t.help_border)
                    .style(Style::default().bg(t.help_bg)),
            ),
        popup_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eq::structs::{
        BuffSlot, CastState as EqCastState, EqClass, SpawnInfo, SpawnType, StandState,
    };
    use crate::tui::app::{ChChainStatus, ClientState, GroupDef, NavClientStatus};
    use dmft_common::nav::NavStatus;
    use ratatui::{Terminal, backend::TestBackend};

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

        assert!(rendered.contains("Char"));
        assert!(rendered.contains("Nav"));
        assert!(rendered.contains("Dbg"));
        assert!(rendered.contains("Toon08"));
        assert!(!rendered.contains("y:"));
    }

    #[test]
    fn overview_render_wide_keeps_full_tabs_and_session_panel() {
        let rendered = render_app(sample_app(), 150, 36);

        assert!(rendered.contains("Characters"));
        assert!(rendered.contains("Navigation"));
        assert!(rendered.contains("Session"));
        assert!(rendered.contains("Toon10"));
    }

    #[test]
    fn help_overlay_small_host_uses_compact_rows() {
        let mut app = sample_app();
        app.help_visible = true;

        let rendered = render_app(app, 80, 24);

        assert!(rendered.contains("Help"));
        assert!(rendered.contains("Active: Characters"));
        assert!(rendered.contains("g - Toggle group roster section"));
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
                is_demo_scripted: false,
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
                    target_id: 42,
                    spell_eta: 0,
                    item_id: 0,
                    spell_slot: 0,
                    remaining_ms: Some(2_500),
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
            cast_state: None,
        }
    }
}
