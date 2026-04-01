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
        let popup_w = (area.width as f32 * 0.6).max(40.0).min(area.width as f32) as u16;
        let popup_h = (area.height as f32 * 0.7).max(15.0).min(area.height as f32) as u16;
        let popup_x = area.x + (area.width.saturating_sub(popup_w)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_h)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_w, popup_h);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            ConfigPanelWidget::new(&app.config_panel_state).accent_color(app.theme.text_accent),
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
        let toast_width = (toast.len() + 4).min(area.width as usize) as u16;
        let toast_x = area.x + area.width.saturating_sub(toast_width) - 1;
        let toast_y = area.y + 1;
        let toast_area = Rect::new(toast_x, toast_y, toast_width, 1);
        frame.render_widget(Clear, toast_area);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {toast} "),
                Style::default().fg(Color::Black).bg(app.theme.text_accent),
            ))),
            toast_area,
        );
    }
}

// ─── Header ──────────────────────────────────────────────────────────────────

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    let client_count = app.clients.len();
    let client_str = if client_count > 0 {
        format!(" {client_count}✕ EQ")
    } else {
        " Not attached".into()
    };

    let selected_str = if let Some(client) = app.active_client() {
        let name = client.local_player.as_ref().map_or_else(
            || "???".into(),
            |p| app.redact_name(&p.displayed_name).into_owned(),
        );
        format!(" [{}/{}] {} ", app.selected_client + 1, client_count, name)
    } else {
        " No client ".into()
    };

    let server_str = format!(" {} ", app.display_server());
    let zone_str = app.active_client().map_or_else(
        || "No Zone".into(),
        |c| {
            if c.zone_name.is_empty() {
                "Unknown Zone".into()
            } else {
                c.zone_name.clone()
            }
        },
    );

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
        Span::styled(" DMFT ", t.header_title),
        Span::styled("│", t.border_dim),
        Span::styled(&client_str, t.header_client_count),
        Span::styled(" │", t.border_dim),
        Span::styled(&selected_str, t.header_selected),
        Span::styled("│ ", t.border_dim),
        Span::styled(format!(" {group_label} "), group_style),
        Span::styled(" │ ", t.border_dim),
        Span::styled(&server_str, Style::default().fg(t.text_server)),
        Span::styled("│ ", t.border_dim),
        Span::styled(format!(" {zone_str} "), t.header_zone),
        Span::styled("│", t.border_dim),
        Span::styled("  ", Style::default()),
    ];
    spans.extend(tabs);

    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(widgets::panel(" DMFT ", t.border_dim, t)),
        area,
    );
}

// ─── Status bar ──────────────────────────────────────────────────────────────

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    // Command mode: full-width input line with syntax hint
    if app.cmd_state.command_mode {
        let input_text = format!(": {}_", app.cmd_state.command_buffer);
        let mut spans = vec![Span::styled(&input_text, t.statusbar_cmd)];

        // Show syntax hint for known commands
        if let Some(hint) = crate::tui::app::command_syntax_hint(&app.cmd_state.command_buffer) {
            spans.push(Span::styled(
                format!("  ({hint})"),
                Style::default().fg(t.text_muted),
            ));
        }

        frame.render_widget(
            Paragraph::new(Line::from(spans)).block(widgets::panel("", t.border_active, t)),
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
            Span::styled("F10", t.statusbar_key),
            Span::styled(" menu  ", t.statusbar_dim),
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
        format!(" {mode_str} "),
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
            format!(" {filter} "),
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
    // Scale to terminal: 70% width (min 50, max 80), 85% height (min 25, max 50)
    let popup_w = (area.width * 70 / 100).clamp(50.min(area.width), 80.min(area.width));
    let popup_h = (area.height * 85 / 100).clamp(25.min(area.height), 50.min(area.height));
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
            Span::styled(format!(" {k:<14}"), key_s),
            Span::styled(v, desc_s),
        ])
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
        Span::styled(" -- keys for this screen shown below", dim_s),
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
        kv("dmft", "config/dmft.toml (main config)"),
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
    let visible_lines = popup_h.saturating_sub(2) as usize;
    let max_scroll = text.len().saturating_sub(visible_lines);
    let scroll = app.help_scroll.min(max_scroll);

    // Build title with scroll indicator
    let title = if max_scroll > 0 {
        let pct = (scroll * 100).checked_div(max_scroll).unwrap_or(0);
        format!(" Help [{pct}%] ")
    } else {
        String::from(" Help ")
    };

    frame.render_widget(
        Paragraph::new(text).scroll((scroll as u16, 0)).block(
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
