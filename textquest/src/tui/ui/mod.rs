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
pub mod dps_bars;
pub mod eq_internals;
pub mod explorer;
pub mod groups;
pub mod map;
pub mod navigation;
pub mod packets;
pub mod spawns;
pub mod widgets;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::tui::app::{ActivePanel, ActiveScreen, App, HelpFocus, ToastLevel};
use crate::tui::command::HelpSection;
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
        ActiveScreen::PacketMonitor => packets::draw_packet_monitor(frame, outer[1], app),
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
    if let Some(toast) = app.toast.as_ref() {
        let (label, style) = match toast.level {
            ToastLevel::Info => (
                "INFO",
                Style::default().fg(Color::Black).bg(app.theme.text_accent),
            ),
            ToastLevel::Success => (
                "OK",
                Style::default().fg(Color::Black).bg(app.theme.hp_high),
            ),
            ToastLevel::Warning => (
                "WARN",
                Style::default()
                    .fg(Color::Black)
                    .bg(app.theme.text_highlight),
            ),
            ToastLevel::Error => (
                "ERR",
                Style::default().fg(Color::White).bg(app.theme.hp_low),
            ),
        };
        let full_text = format!("{label} {}", toast.message);
        let toast_text = truncate_inline(&full_text, area.width.saturating_sub(4) as usize);
        let toast_width = (toast_text.chars().count() + 2).min(area.width as usize) as u16;
        let toast_x = area.x + area.width.saturating_sub(toast_width).saturating_sub(1);
        let toast_y = area.y + 1;
        let toast_area = Rect::new(toast_x, toast_y, toast_width, 1);
        frame.render_widget(Clear, toast_area);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(format!(" {toast_text} "), style))),
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
            ActiveScreen::PacketMonitor => "Pkt",
        },
        WidthClass::Narrow => match screen {
            ActiveScreen::Overview => "1",
            ActiveScreen::Tactical => "2",
            ActiveScreen::Navigation => "3",
            ActiveScreen::Debug => "4",
            ActiveScreen::PacketMonitor => "5",
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
    let block = widgets::panel(" TextQuest ", t.border_dim, t);
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
                ("+/-", "zoom"),
                ("n", "mesh"),
                ("v", "view"),
                ("?", "help"),
            ],
            WidthClass::Medium => &[
                ("1-4", "screen"),
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
                ("1-4", "screen"),
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
    let ch_label = app.ch_chain_status.as_ref().map(|chain| {
        format!(
            "{}x@{:.1}s{}",
            chain.members,
            chain.interval_secs,
            if chain.is_adaptive { "A" } else { "" }
        )
    });

    let filter = app.spawns_state.spawn_type_filter.label();
    if let Some(ma) = ma_label {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::raw(" "),
                Span::styled(format!(" MA {ma} "), Style::default().fg(t.text_highlight)),
            ],
            max_width,
        );
    }
    if let Some(mt) = mt_label {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::raw(" "),
                Span::styled(format!(" MT {mt} "), Style::default().fg(t.hp_low)),
            ],
            max_width,
        );
    }
    if let Some(ch) = ch_label {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::raw(" "),
                Span::styled(format!(" CH {ch} "), Style::default().fg(t.text_accent)),
            ],
            max_width,
        );
    }
    if app.active_group.is_some() {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::raw(" "),
                Span::styled(
                    format!(" {} ", truncate_inline(&app.group_focus_label(), 10)),
                    Style::default().fg(Color::Black).bg(t.text_accent),
                ),
            ],
            max_width,
        );
    }
    if filter != "All" && width_class != WidthClass::Narrow {
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
    if app.automation_paused {
        let _ = push_segment_if_fits(
            &mut spans,
            vec![
                Span::raw(" "),
                Span::styled(
                    " \u{23f8} PAUSED ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
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
    match app.active_screen {
        ActiveScreen::Overview => {
            push_heading(&mut rows, None, "Dashboard Controls");
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
        }
        ActiveScreen::Tactical => {
            push_heading(&mut rows, None, "Map Controls");
            push_kv(&mut rows, None, "+ / -", "Zoom in / out");
            push_kv(&mut rows, None, "Arrows", "Pan the map viewport");
            push_kv(
                &mut rows,
                None,
                "< / >",
                "Adjust Z-depth slice (height filter)",
            );
            push_kv(
                &mut rows,
                None,
                "v",
                "Cycle viewport: Auto / Local / Global",
            );
            push_kv(&mut rows, None, "n", "Toggle navmesh overlay data");
            push_kv(&mut rows, None, "m", "Maximize / restore map panel");
            push_kv(&mut rows, None, "Home", "Reset zoom, pan, and viewport");
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
                "x",
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
            push_kv(&mut rows, None, "Shift+N", "Toggle NPC spawn markers");
            push_kv(&mut rows, None, "Shift+P", "Toggle PC spawn markers");
            push_kv(&mut rows, None, "Shift+C", "Toggle corpse markers");
            push_kv(&mut rows, None, "Shift+G", "Toggle ground-spawn markers");
            push_kv(&mut rows, None, "Shift+T", "Toggle pet markers");
            push_kv(
                &mut rows,
                None,
                "Shift+R",
                "Toggle named (rare) NPC markers",
            );
            push_kv(
                &mut rows,
                None,
                "Shift+U",
                "Toggle untargetable spawn markers",
            );
            rows.push(help_row(None, HelpCell::Text(String::new())));
            push_heading(&mut rows, None, "Spawn List");
            push_kv(&mut rows, None, "/", "Search spawns by name");
            push_kv(&mut rows, None, "f", "Cycle filter: All / PC / NPC / Named");
            push_kv(&mut rows, None, "j/k", "Navigate spawn list");
            push_kv(&mut rows, None, "t", "Navigate to selected spawn");
            push_kv(&mut rows, None, "a", "Target selected spawn (/target)");
        }
        ActiveScreen::Navigation => {
            push_kv(
                &mut rows,
                None,
                "j/k or Up/Down",
                "Navigate client nav statuses",
            );
            push_kv(&mut rows, None, "Enter", "Jump back to the map panel");
            push_kv(&mut rows, None, ":nav <dest>", "Send a navigation command");
        }
        ActiveScreen::Debug => {
            push_heading(&mut rows, None, "Debug Controls");
            push_kv(&mut rows, None, "j/k", "Navigate spawns or scroll hex dump");
            push_kv(&mut rows, None, "Enter", "Inspect the selected spawn");
            push_kv(&mut rows, None, "/", "Search spawns by name");
            push_kv(&mut rows, None, "f", "Cycle spawn filter");
            push_kv(&mut rows, None, "a", "Toggle hex dump annotations");
            push_kv(&mut rows, None, "c", "Cycle EQ Internals category filter");
        }
        ActiveScreen::PacketMonitor => {
            push_kv(&mut rows, None, "Space", "Pause / resume packet capture");
            push_kv(&mut rows, None, "↑/↓", "Scroll packet log");
            push_kv(&mut rows, None, "c", "Clear captured packets");
        }
    }
    rows.push(help_row(None, HelpCell::Text(String::new())));

    push_heading(&mut rows, None, "Global Controls");
    push_kv(
        &mut rows,
        None,
        "1-4",
        "Switch screen: Dashboard / Map / Nav / Debug",
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
    let popup_area = centered_popup(area, 80, 85, 50, 18, 100, 50, 1);
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
        format!(" Help [{}%] ", (scroll * 100) / max_scroll.max(1))
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
        BuffSlot, CastDurationSource, CastState as EqCastState, EqClass, SpawnInfo, SpawnType,
        StandState,
    };
    use crate::tui::app::{ChChainStatus, ClientState, GroupDef, NavClientStatus};
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
    fn navigation_render_surfaces_route_state_and_blockers() {
        let mut app = sample_app();
        app.active_screen = ActiveScreen::Navigation;
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
        assert!(rendered.contains("Active: Characters"));
        assert!(rendered.contains("Dashboard Controls"));
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
