use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
};

use super::app::{App, ActivePanel};
use crate::eq::structs::{SpawnInfo, SpawnType};

/// Main render function — draws all panels.
pub fn draw(frame: &mut Frame, app: &App) {
    // Top-level layout: header, body, footer
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header bar
            Constraint::Min(10),   // Body
            Constraint::Length(3), // Status bar
        ])
        .split(frame.area());

    draw_header(frame, outer[0], app);
    draw_body(frame, outer[1], app);
    draw_status_bar(frame, outer[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let pid_str = app.attached_pid
        .map(|p| format!("PID:{}", p))
        .unwrap_or_else(|| "Not attached".into());

    let base_str = if app.eq_base != 0 {
        format!("Base:{:#x}", app.eq_base)
    } else {
        String::new()
    };

    let player_str = app.local_player.as_ref()
        .map(|p| format!("{} ({}) Lv{}", p.displayed_name, p.class_str(), p.level))
        .unwrap_or_else(|| "No character".into());

    let tick_str = format!(" │ Tick:{} ", app.tick_count);

    let header = Paragraph::new(Line::from(vec![
        Span::styled(" FROSTREAVER ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("│ "),
        Span::styled(&pid_str, Style::default().fg(Color::Yellow)),
        Span::raw(" "),
        Span::styled(&base_str, Style::default().fg(Color::DarkGray)),
        Span::raw(" │ "),
        Span::styled(&player_str, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(&tick_str),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" EQ Memory Debugger "));

    frame.render_widget(header, area);
}

fn draw_body(frame: &mut Frame, area: Rect, app: &App) {
    // Body: left side (player + target + hex), right side (spawn list)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Left: player info + hex
            Constraint::Percentage(60), // Right: spawn list
        ])
        .split(area);

    // Left column: player, target, hex dump
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Player info
            Constraint::Length(8),  // Target info
            Constraint::Min(5),    // Hex dump
        ])
        .split(cols[0]);

    draw_player_panel(frame, left[0], app);
    draw_target_panel(frame, left[1], app);
    draw_hex_panel(frame, left[2], app);

    // Right column: spawn list
    draw_spawn_list(frame, cols[1], app);
}

fn draw_player_panel(frame: &mut Frame, area: Rect, app: &App) {
    draw_spawn_panel(frame, area, &app.local_player, " Local Player ", Color::Green, "Not logged in");
}

fn draw_target_panel(frame: &mut Frame, area: Rect, app: &App) {
    draw_spawn_panel(frame, area, &app.target, " Current Target ", Color::Red, "No target");
}

fn draw_spawn_panel(
    frame: &mut Frame,
    area: Rect,
    spawn: &Option<SpawnInfo>,
    title: &str,
    border_color: Color,
    empty_msg: &str,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(border_color));

    if let Some(info) = spawn {
        let lines = spawn_info_lines(info);
        let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    } else {
        let paragraph = Paragraph::new(empty_msg)
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
    }
}

fn spawn_info_lines(spawn: &SpawnInfo) -> Vec<Line<'_>> {
    let hp_pct = spawn.hp_pct();
    let hp_color = if hp_pct > 75.0 {
        Color::Green
    } else if hp_pct > 25.0 {
        Color::Yellow
    } else {
        Color::Red
    };

    vec![
        Line::from(vec![
            Span::styled(&spawn.displayed_name, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(
                format!("{} Lv{}", spawn.class_str(), spawn.level),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(format!("  [{}]", spawn.spawn_type)),
        ]),
        Line::from(vec![
            Span::raw("HP: "),
            Span::styled(
                format!("{}/{} ({:.0}%)", spawn.hp_current, spawn.hp_max, hp_pct),
                Style::default().fg(hp_color),
            ),
        ]),
        Line::from(vec![
            Span::raw("Mana: "),
            Span::styled(
                format!("{}/{}", spawn.mana_current, spawn.mana_max),
                Style::default().fg(Color::Blue),
            ),
            Span::raw(format!("  End: {}/{}", spawn.endurance_current, spawn.endurance_max)),
        ]),
        Line::from(vec![
            Span::raw("Pos: "),
            Span::styled(
                format!("({:.1}, {:.1}, {:.1})", spawn.y, spawn.x, spawn.z),
                Style::default().fg(Color::Magenta),
            ),
            Span::raw(format!("  Hdg: {:.1}", spawn.heading)),
        ]),
        Line::from(vec![
            Span::raw(format!("ID: {}  Name: {}", spawn.spawn_id, spawn.name)),
        ]),
    ]
}

fn draw_hex_panel(frame: &mut Frame, area: Rect, app: &App) {
    let is_active = app.active_panel == ActivePanel::HexDump;
    let border_color = if is_active { Color::Yellow } else { Color::DarkGray };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Hex Dump — {} ", app.hex_label))
        .border_style(Style::default().fg(border_color));

    if app.hex_data.is_empty() {
        let paragraph = Paragraph::new("Select a spawn and press Enter to inspect memory")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    // Format hex dump: address | hex bytes | ascii
    let inner_height = area.height.saturating_sub(2) as usize;
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(inner_height);

    for row in 0..inner_height {
        let offset = row * 16;
        if offset >= app.hex_data.len() {
            break;
        }

        let addr = app.hex_address + offset;
        let end = (offset + 16).min(app.hex_data.len());
        let chunk = &app.hex_data[offset..end];

        let hex_str: String = chunk
            .iter()
            .map(|b| format!("{:02x} ", b))
            .collect();

        let ascii_str: String = chunk
            .iter()
            .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
            .collect();

        lines.push(Line::from(vec![
            Span::styled(format!("{:08x}", addr), Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(format!("{:<48}", hex_str), Style::default().fg(Color::White)),
            Span::raw(" "),
            Span::styled(ascii_str, Style::default().fg(Color::Yellow)),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_spawn_list(frame: &mut Frame, area: Rect, app: &App) {
    let is_active = app.active_panel == ActivePanel::SpawnList;
    let border_color = if is_active { Color::Cyan } else { Color::DarkGray };

    let filtered = app.filtered_spawns();
    let title = if app.spawn_filter.is_empty() {
        format!(" Spawn List ({}) ", filtered.len())
    } else {
        format!(" Spawn List ({}) filter: \"{}\" ", filtered.len(), app.spawn_filter)
    };

    let header = Row::new(vec![
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Cls").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Lv").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("HP%").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("ID").style(Style::default().add_modifier(Modifier::BOLD)),
    ]).height(1);

    let rows: Vec<Row> = filtered
        .iter()
        .enumerate()
        .map(|(i, spawn)| {
            let is_selected = i == app.spawn_selected;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                spawn_row_style(spawn)
            };

            Row::new(vec![
                Cell::from(spawn.spawn_type.to_string()),
                Cell::from(spawn.displayed_name.as_str()),
                Cell::from(spawn.class_str()),
                Cell::from(spawn.level.to_string()),
                Cell::from(format!("{:.0}%", spawn.hp_pct())),
                Cell::from(spawn.spawn_id.to_string()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(7),  // Type
            Constraint::Min(20),   // Name
            Constraint::Length(4),  // Class
            Constraint::Length(4),  // Level
            Constraint::Length(6),  // HP%
            Constraint::Length(8),  // ID
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color)),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_widget(table, area);
}

fn spawn_row_style(spawn: &SpawnInfo) -> Style {
    match spawn.spawn_type {
        SpawnType::Player => Style::default().fg(Color::Green),
        SpawnType::Npc => Style::default().fg(Color::White),
        SpawnType::Corpse => Style::default().fg(Color::DarkGray),
        SpawnType::Unknown(_) => Style::default().fg(Color::Red),
    }
}

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let keybinds = " q:Quit | Tab:Switch Panel | j/k:Navigate | Enter:Inspect | Esc:Clear Filter | /:Filter ";

    let status = Paragraph::new(Line::from(vec![
        Span::styled(&app.status_message, Style::default().fg(Color::Yellow)),
        Span::raw("  │  "),
        Span::styled(keybinds, Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(status, area);
}
