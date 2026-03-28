use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

use super::app::{ActivePanel, App};
use crate::eq::structs::{SpawnInfo, SpawnType};

/// Main render function — draws all panels.
pub fn draw(frame: &mut Frame, app: &App) {
    // Top-level layout: header, body, footer
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header bar
            Constraint::Min(10),   // Body
            Constraint::Length(3), // Status bar
        ])
        .split(frame.area());

    draw_header(frame, outer[0], app);
    draw_body(frame, outer[1], app);
    draw_status_bar(frame, outer[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let client_count = app.clients.len();
    let client_str = if client_count > 0 {
        format!("{}x EQ", client_count)
    } else {
        "Not attached".into()
    };

    let selected_str = if let Some(client) = app.active_client() {
        let player_name = client
            .local_player
            .as_ref()
            .map(|p| p.displayed_name.as_str())
            .unwrap_or("???");
        format!(
            "[{}/{}] {}",
            app.selected_client + 1,
            client_count,
            player_name
        )
    } else {
        String::from("No client selected")
    };

    let server_str = format!(" {} ", app.server_name);
    let _tick_str = format!(" Tick:{} ", app.tick_count);

    // Get zone from active client
    let zone_str = app.active_client()
        .map(|c| if c.zone_name.is_empty() { "Unknown Zone".to_string() } else { c.zone_name.clone() })
        .unwrap_or_else(|| "No Zone".to_string());

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            &client_str,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(
            &selected_str,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(&server_str, Style::default().fg(Color::Magenta)),
        Span::raw("| "),
        Span::styled(
            format!(" {} ", zone_str),
            Style::default().fg(Color::White),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" EQ Multibox Controller "),
    );

    frame.render_widget(header, area);
}

fn draw_body(frame: &mut Frame, area: Rect, app: &App) {
    // Body: left side (character summary + hex), right side (spawn list)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Left: character panels
            Constraint::Percentage(60), // Right: spawn list
        ])
        .split(area);

    // Left column: character summary, target, hex dump
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),    // Character summary (multi-client)
            Constraint::Length(8), // Target info (selected client)
            Constraint::Length(8), // Hex dump
        ])
        .split(cols[0]);

    draw_character_summary(frame, left[0], app);
    draw_target_panel(frame, left[1], app);
    draw_hex_panel(frame, left[2], app);

    // Right column: spawn list
    draw_spawn_list(frame, cols[1], app);
}

/// Draw the multi-client character summary panel with Tamagotchi sprites.
fn draw_character_summary(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Characters ({}) ", app.clients.len()))
        .border_style(Style::default().fg(Color::Green));

    if app.clients.is_empty() {
        let paragraph = Paragraph::new("No characters connected")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Each character gets 2 lines: name/class/level/hp + sprite + zone
    let mut lines: Vec<Line<'_>> = Vec::new();

    for (i, client) in app.clients.iter().enumerate() {
        let is_selected = i == app.selected_client;
        let marker = if is_selected { ">" } else { " " };

        if let Some(player) = &client.local_player {
            let hp_pct = player.hp_pct();
            let hp_color = hp_color(hp_pct);
            let mana_pct = player.mana_pct();

            let sprite_label = player.stand_state.label();
            let sprite_color = match player.stand_state {
                crate::eq::structs::StandState::Dead => Color::Red,
                crate::eq::structs::StandState::Sitting => Color::Yellow,
                crate::eq::structs::StandState::Feigned => Color::Magenta,
                crate::eq::structs::StandState::Frozen => Color::Blue,
                _ => Color::Green,
            };

            let name_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            };

            // Line 1: marker name class level | HP% | zone
            lines.push(Line::from(vec![
                Span::styled(
                    marker,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:<14}", player.displayed_name), name_style),
                Span::styled(
                    format!("{:>3} ", player.class_str()),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{:>2} ", player.level),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("HP:{:>3.0}% ", hp_pct),
                    Style::default().fg(hp_color),
                ),
                if player.mana_max > 0 {
                    Span::styled(
                        format!("MP:{:>3.0}% ", mana_pct),
                        Style::default().fg(Color::Blue),
                    )
                } else {
                    Span::raw("       ")
                },
                Span::styled(
                    format!("[{}]", sprite_label),
                    Style::default().fg(sprite_color),
                ),
                Span::raw(" "),
                Span::styled(&client.zone_name, Style::default().fg(Color::DarkGray)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("PID {} — not logged in", client.pid),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    // Add a blank line then the selected character's sprite art
    if let Some(client) = app.active_client() {
        if let Some(player) = &client.local_player {
            lines.push(Line::from(""));
            let sprite_color = match player.stand_state {
                crate::eq::structs::StandState::Dead => Color::Red,
                crate::eq::structs::StandState::Sitting => Color::Yellow,
                crate::eq::structs::StandState::Feigned => Color::Magenta,
                _ => Color::Green,
            };

            // Render sprite lines
            for sprite_line in player.stand_state.sprite().lines() {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(sprite_line, Style::default().fg(sprite_color)),
                    Span::raw(format!(
                        "  {} ({}) Lv{} — {}",
                        player.displayed_name,
                        player.class_str(),
                        player.level,
                        client.zone_name,
                    )),
                ]));
            }
        }
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn draw_target_panel(frame: &mut Frame, area: Rect, app: &App) {
    let target = app.active_client().and_then(|c| c.target.as_ref());
    draw_spawn_panel(
        frame,
        area,
        target,
        " Current Target ",
        Color::Red,
        "No target",
    );
}

fn draw_spawn_panel(
    frame: &mut Frame,
    area: Rect,
    spawn: Option<&SpawnInfo>,
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
    let hp_col = hp_color(hp_pct);

    vec![
        Line::from(vec![
            Span::styled(
                &spawn.displayed_name,
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} Lv{}", spawn.class_str(), spawn.level),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(format!("  [{}]", spawn.spawn_type)),
            Span::raw(format!("  {}", spawn.stand_state)),
        ]),
        Line::from(vec![
            Span::raw("HP: "),
            Span::styled(
                format!("{}/{} ({:.0}%)", spawn.hp_current, spawn.hp_max, hp_pct),
                Style::default().fg(hp_col),
            ),
        ]),
        Line::from(vec![
            Span::raw("Mana: "),
            Span::styled(
                format!("{}/{}", spawn.mana_current, spawn.mana_max),
                Style::default().fg(Color::Blue),
            ),
            Span::raw(format!(
                "  End: {}/{}",
                spawn.endurance_current, spawn.endurance_max
            )),
        ]),
        Line::from(vec![
            Span::raw("Pos: "),
            Span::styled(
                format!("({:.1}, {:.1}, {:.1})", spawn.y, spawn.x, spawn.z),
                Style::default().fg(Color::Magenta),
            ),
            Span::raw(format!("  Hdg: {:.1}", spawn.heading)),
        ]),
        Line::from(vec![Span::raw(format!(
            "ID: {}  Name: {}",
            spawn.spawn_id, spawn.name
        ))]),
    ]
}

fn hp_color(hp_pct: f64) -> Color {
    if hp_pct > 75.0 {
        Color::Green
    } else if hp_pct > 25.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn draw_hex_panel(frame: &mut Frame, area: Rect, app: &App) {
    let is_active = app.active_panel == ActivePanel::HexDump;
    let border_color = if is_active {
        Color::Yellow
    } else {
        Color::DarkGray
    };

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

        let hex_str: String = chunk.iter().map(|b| format!("{:02x} ", b)).collect();

        let ascii_str: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();

        lines.push(Line::from(vec![
            Span::styled(
                format!("{:08x}", addr),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{:<48}", hex_str),
                Style::default().fg(Color::White),
            ),
            Span::raw(" "),
            Span::styled(ascii_str, Style::default().fg(Color::Yellow)),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_spawn_list(frame: &mut Frame, area: Rect, app: &App) {
    let is_active = app.active_panel == ActivePanel::SpawnList;
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let filtered = app.filtered_spawns();

    // Show which client's spawns we're viewing
    let client_label = app
        .active_client()
        .and_then(|c| c.local_player.as_ref())
        .map(|p| p.displayed_name.as_str())
        .unwrap_or("???");

    let title = if app.spawn_filter.is_empty() {
        format!(" Spawns: {} ({}) ", client_label, filtered.len())
    } else {
        format!(
            " Spawns: {} ({}) filter: \"{}\" ",
            client_label,
            filtered.len(),
            app.spawn_filter
        )
    };

    let header = Row::new(vec![
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Cls").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Lv").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("HP%").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("ID").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .height(1);

    let rows: Vec<Row> = filtered
        .iter()
        .enumerate()
        .map(|(i, spawn)| {
            let is_selected = i == app.spawn_selected;
            let style = if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
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
            Constraint::Length(7), // Type
            Constraint::Min(20),   // Name
            Constraint::Length(4), // Class
            Constraint::Length(4), // Level
            Constraint::Length(6), // HP%
            Constraint::Length(8), // ID
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
    let keybinds =
        " q:Quit | Tab:Panel | [/]:Client | j/k:Nav | Enter:Inspect | Esc:Clear | /:Search | f:Filter(All>PC>NPC) ";

    let status = Paragraph::new(Line::from(vec![
        Span::styled(&app.status_message, Style::default().fg(Color::Yellow)),
        Span::raw("  |  "),
        Span::styled(keybinds, Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(status, area);
}
