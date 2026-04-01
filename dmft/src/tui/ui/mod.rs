//! TUI renderer — entry point and global chrome (header, status bar, help overlay).
//!
//! Each screen lives in its own sub-module:
//! - [`dashboard`]   — character grid + health gauges + session stats
//! - [`spawns`]      — filterable spawn list + hex dump
//! - [`map`]         — zone map + named tracker
//! - [`groups`]      — per-group panels with buff timer columns
//! - [`navigation`]  — nav status + commands reference
//! - [`widgets`]     — shared helpers (`panel`, `themed_header_row`, colour fns …)

pub mod dashboard;
pub mod groups;
pub mod map;
pub mod navigation;
pub mod spawns;
pub mod widgets;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::tui::app::{ActivePanel, ActiveScreen, App};

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

    if app.help_visible {
        draw_help_overlay(frame, frame.area(), app);
    }
}

// ─── Header ──────────────────────────────────────────────────────────────────

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    let client_count = app.clients.len();
    let client_str = if client_count > 0 {
        format!(" {}✕ EQ", client_count)
    } else {
        " Not attached".into()
    };

    let selected_str = if let Some(client) = app.active_client() {
        let name = client
            .local_player
            .as_ref()
            .map(|p| app.redact_name(&p.displayed_name).into_owned())
            .unwrap_or_else(|| "???".into());
        format!(" [{}/{}] {} ", app.selected_client + 1, client_count, name)
    } else {
        " No client ".into()
    };

    let server_str = format!(" {} ", app.display_server());
    let zone_str = app
        .active_client()
        .map(|c| {
            if c.zone_name.is_empty() {
                "Unknown Zone".into()
            } else {
                c.zone_name.clone()
            }
        })
        .unwrap_or_else(|| "No Zone".into());

    // Tab bar — current screen is highlighted with accent bg
    let mut tabs: Vec<Span<'_>> = vec![Span::raw("  ")];
    for screen in &ActiveScreen::ALL {
        let is_active = *screen == app.active_screen;
        let label = format!(" {} ", screen.label());
        tabs.push(if is_active {
            Span::styled(label, t.tab_active)
        } else {
            Span::styled(label, t.tab_inactive)
        });
        tabs.push(Span::raw(" "));
    }

    // Group indicator — bold + accent when focused to make it prominent
    let group_label = app.group_focus_label();
    let group_style = if app.active_group.is_some() {
        t.header_group_active.add_modifier(Modifier::BOLD)
    } else {
        t.header_group
    };

    let mut spans: Vec<Span<'_>> = vec![
        Span::styled(" FROST ", t.header_title),
        Span::styled("│", t.border_dim),
        Span::styled(&client_str, t.header_client_count),
        Span::styled(" │", t.border_dim),
        Span::styled(&selected_str, t.header_selected),
        Span::styled("│ ", t.border_dim),
        Span::styled(format!(" {} ", group_label), group_style),
        Span::styled(" │ ", t.border_dim),
        Span::styled(&server_str, Style::default().fg(t.text_server)),
        Span::styled("│ ", t.border_dim),
        Span::styled(format!(" {} ", zone_str), t.header_zone),
        Span::styled("│", t.border_dim),
        Span::styled("  ", Style::default()),
    ];
    spans.extend(tabs);

    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(widgets::panel(" Frostreaver ", t.border_dim, t)),
        area,
    );
}

// ─── Status bar ──────────────────────────────────────────────────────────────

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    // Command mode: full-width input line
    if app.cmd_state.command_mode {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(": {}_", app.cmd_state.command_buffer),
                t.statusbar_cmd,
            )))
            .block(widgets::panel("", t.border_active, t)),
            area,
        );
        return;
    }

    // Split: left = message + hints, right = status badges
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(32)])
        .split(area);

    // ── Left pane ─────────────────────────────────────────────────────
    let hints: Vec<Span<'_>> = if app.active_panel == ActivePanel::TacticalMap {
        vec![
            Span::styled("1-4", t.statusbar_key),
            Span::styled(" screen  ", t.statusbar_dim),
            Span::styled("Tab", t.statusbar_key),
            Span::styled(" pane  ", t.statusbar_dim),
            Span::styled("[ ]", t.statusbar_key),
            Span::styled(" client  ", t.statusbar_dim),
            Span::styled("v", t.statusbar_key),
            Span::styled(" view  ", t.statusbar_dim),
            Span::styled("n", t.statusbar_key),
            Span::styled(" mesh  ", t.statusbar_dim),
            Span::styled("PgUp/Dn", t.statusbar_key),
            Span::styled(" zoom  ", t.statusbar_dim),
            Span::styled("←↑↓→", t.statusbar_key),
            Span::styled(" pan  ", t.statusbar_dim),
            Span::styled("+/-", t.statusbar_key),
            Span::styled(" depth  ", t.statusbar_dim),
            Span::styled("Home", t.statusbar_key),
            Span::styled(" reset  ", t.statusbar_dim),
            Span::styled("m", t.statusbar_key),
            Span::styled(" map  ", t.statusbar_dim),
            Span::styled("?", t.statusbar_key),
            Span::styled(" help", t.statusbar_dim),
        ]
    } else {
        vec![
            Span::styled("1-4", t.statusbar_key),
            Span::styled(" screen  ", t.statusbar_dim),
            Span::styled("⇧1-6", t.statusbar_key),
            Span::styled(" group  ", t.statusbar_dim),
            Span::styled("Tab", t.statusbar_key),
            Span::styled(" pane  ", t.statusbar_dim),
            Span::styled("[ ]", t.statusbar_key),
            Span::styled(" client  ", t.statusbar_dim),
            Span::styled("g/v", t.statusbar_key),
            Span::styled(" sections  ", t.statusbar_dim),
            Span::styled("z", t.statusbar_key),
            Span::styled(" collapse  ", t.statusbar_dim),
            Span::styled("/", t.statusbar_key),
            Span::styled(" search  ", t.statusbar_dim),
            Span::styled("f", t.statusbar_key),
            Span::styled(" filter  ", t.statusbar_dim),
            Span::styled("+/-", t.statusbar_key),
            Span::styled(" depth  ", t.statusbar_dim),
            Span::styled("m", t.statusbar_key),
            Span::styled(" map  ", t.statusbar_dim),
            Span::styled("T", t.statusbar_key),
            Span::styled(" theme  ", t.statusbar_dim),
            Span::styled("?", t.statusbar_key),
            Span::styled(" help", t.statusbar_dim),
        ]
    };

    let left_spans: Vec<Span<'_>> = std::iter::once(Span::raw(" "))
        .chain(std::iter::once(Span::styled(
            app.status_message.as_str(),
            t.statusbar_message,
        )))
        .chain(std::iter::once(Span::styled("  │  ", t.statusbar_dim)))
        .chain(hints)
        .collect();

    frame.render_widget(
        Paragraph::new(Line::from(left_spans)).block(widgets::panel("", t.border_dim, t)),
        cols[0],
    );

    // ── Right pane: colored badges ─────────────────────────────────────
    let mode_str = format!("{}", app.operating_mode);
    let mode_bg = match mode_str.as_str() {
        "Camp" => t.mode_camp,
        "Hunt" => t.mode_hunt,
        _ => t.text_muted,
    };

    let mut right: Vec<Span<'_>> = vec![];

    // Mode badge
    right.push(Span::styled(
        format!(" {} ", mode_str),
        Style::default()
            .fg(Color::Black)
            .bg(mode_bg)
            .add_modifier(Modifier::BOLD),
    ));
    right.push(Span::raw(" "));

    // Filter badge (only when non-default)
    let filter = app.spawns_state.spawn_type_filter.label();
    if filter != "All" {
        right.push(Span::styled(
            format!(" {} ", filter),
            Style::default()
                .fg(Color::Black)
                .bg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ));
        right.push(Span::raw(" "));
    }

    // Privacy badge
    if app.privacy_mode {
        right.push(Span::styled(" PRIVATE ", t.statusbar_badge));
        right.push(Span::raw(" "));
    }

    // Active group badge
    if let Some(idx) = app.active_group {
        right.push(Span::styled(
            format!(" G{} ", idx + 1),
            Style::default().fg(Color::Black).bg(t.text_accent),
        ));
        right.push(Span::raw(" "));
    }

    // Theme label (dim)
    right.push(Span::styled(
        format!(" {} ", app.theme_kind.label()),
        t.statusbar_dim,
    ));

    frame.render_widget(
        Paragraph::new(Line::from(right)).block(widgets::panel("", t.border_dim, t)),
        cols[1],
    );
}

// ─── Help overlay ─────────────────────────────────────────────────────────────

fn draw_help_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    // Scale to terminal: 60% width (min 40, max 60), 80% height (min 20, max 40)
    let popup_w = (area.width * 60 / 100).clamp(40.min(area.width), 60.min(area.width));
    let popup_h = (area.height * 80 / 100).clamp(20.min(area.height), 40.min(area.height));
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup_area = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup_area);

    let key_s = t.help_key;
    let desc_s = t.help_desc;
    let head_s = t.help_heading;
    let dim_s = t.help_dim;

    let kv = |k: &'static str, v: &'static str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!(" {:<12}", k), key_s),
            Span::styled(v, desc_s),
        ])
    };

    let text = vec![
        Line::from(Span::styled(" Keybindings", head_s)),
        Line::from(""),
        kv("1-4", "Characters, Map, Navigation, Debug"),
        kv("Shift+1-6", "Focus group G1–G6"),
        kv("Shift+0", "All groups"),
        kv("Tab", "Cycle focused pane"),
        kv("[ ]", "Cycle clients"),
        kv("/", "Search spawns"),
        kv("f", "Filter spawn type"),
        kv("Enter", "Expand or open focused detail"),
        kv("g", "Toggle group section"),
        kv("v", "Toggle scope section"),
        kv("z", "Collapse focused section"),
        kv("+/-", "Adjust Tactical Z slice"),
        kv("PgUp/PgDn", "Zoom Tactical map"),
        kv("Arrow keys", "Pan Tactical map"),
        kv("Home", "Reset Tactical viewport"),
        kv("v (Map pane)", "Cycle auto/local/global view"),
        kv("n (Map pane)", "Toggle navmesh overlay"),
        kv("m", "Maximize Tactical map"),
        kv("p", "Privacy mode"),
        kv("T", "Cycle theme"),
        kv(":", "Command mode"),
        kv("?", "This help"),
        kv("q", "Quit"),
        Line::from(""),
        Line::from(Span::styled(" Commands  (:cmd)", head_s)),
        Line::from(""),
        kv("<name> /cmd", "Send to character"),
        kv("@<name> /cmd", "Force direct target"),
        kv("G1-G6 /cmd", "Send to group"),
        kv("all /cmd", "Broadcast"),
        kv("camp <sub>", "start|stop|list|add|rm"),
        kv("nav <dest>", "Camp, coords, or slash fallback"),
        kv("track <n>", "Track spawn"),
        kv("ma <name>", "Set Main Assist"),
        kv("mt <name>", "Set Main Tank"),
        kv("engage", "Start combat"),
        kv("disengage", "Stop combat"),
        kv("invite <n>", "Group invite"),
        kv("accept", "Accept invite"),
        kv("mode camp", "Camp mode"),
        kv("mode hunt", "Hunt mode"),
        Line::from(""),
        Line::from(Span::styled(" Status Glyphs", head_s)),
        Line::from(""),
        kv("⚔ / ✚ / ✦", "Fight, Heal, Cast"),
        kv("➜ / ✓ / !", "Navigate, Arrived, Stuck"),
        kv("☾ / ⇣ / ⌕", "Sit, Feign, Loot"),
        Line::from(""),
        Line::from(Span::styled(" CH Chain", head_s)),
        Line::from(""),
        kv("ch start", "<pids> <interval>"),
        kv("ch stop", "Stop CH chain"),
        kv("ch add <pid>", "Add cleric"),
        kv("ch rm <pid>", "Remove cleric"),
        kv("ch interval", "<seconds>"),
        kv("ch adaptive", "on|off"),
        Line::from(""),
        Line::from(Span::styled(" Press ? or Esc to close", dim_s)),
    ];

    frame.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(t.border_type)
                .title(Span::styled(" Help ", t.help_heading))
                .border_style(t.help_border)
                .style(Style::default().bg(t.help_bg)),
        ),
        popup_area,
    );
}
