//! Shared widget-building helpers used across all screen modules.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row},
};

use crate::eq::structs::{SpawnInfo, SpawnType};
use crate::tui::theme::Theme;

// ─── Block / panel helper ────────────────────────────────────────────────────

/// Build a `Block` with the project's standard chrome: border type + style + title.
/// Using this everywhere ensures every panel switches to rounded borders together.
pub fn panel<'a>(
    title: impl Into<ratatui::text::Line<'a>>,
    border_style: Style,
    t: &Theme,
) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(t.border_type)
        .title(title)
        .border_style(border_style)
}

// ─── Table helpers ───────────────────────────────────────────────────────────

/// Build a table header row with all cells styled using `theme.table_header`.
pub fn themed_header_row<'a>(cells: Vec<&'a str>, t: &Theme) -> Row<'a> {
    Row::new(
        cells
            .into_iter()
            .map(|c| Cell::from(c).style(t.table_header))
            .collect::<Vec<_>>(),
    )
    .height(1)
    .bottom_margin(0)
}

// ─── Color helpers ───────────────────────────────────────────────────────────

pub fn hp_color(hp_pct: f64, t: &Theme) -> Color {
    if hp_pct > 75.0 {
        t.hp_high
    } else if hp_pct > 25.0 {
        t.hp_mid
    } else {
        t.hp_low
    }
}

pub fn stand_state_color(state: &crate::eq::structs::StandState, t: &Theme) -> Color {
    use crate::eq::structs::StandState;
    match state {
        StandState::Dead => t.state_dead,
        StandState::Sitting => t.state_sitting,
        StandState::Feigned => t.state_feigned,
        StandState::Frozen => t.state_frozen,
        _ => t.state_normal,
    }
}

pub fn spawn_type_color(st: &SpawnType, t: &Theme) -> Color {
    match st {
        SpawnType::Player => t.spawn_pc,
        SpawnType::Npc => t.spawn_npc,
        SpawnType::Corpse => t.spawn_corpse,
        SpawnType::Unknown(_) => t.spawn_unknown,
    }
}

/// EQ con color — level delta from player perspective.
/// delta = mob_level - player_level
pub fn con_color(player_level: u8, mob_level: u8, t: &Theme) -> Color {
    let delta = mob_level as i16 - player_level as i16;
    match delta {
        d if d >= 4 => t.con_red,
        1..=3 => t.con_yellow,
        0 => t.con_white,
        -3..=-1 => t.con_light_blue,
        -6..=-4 => t.con_blue,
        _ => t.con_green,
    }
}

pub fn spawn_row_style(
    spawn: &SpawnInfo,
    player_level: Option<u8>,
    t: &Theme,
) -> ratatui::style::Style {
    match spawn.spawn_type {
        SpawnType::Player => Style::default().fg(t.spawn_pc),
        SpawnType::Npc => {
            let color = player_level
                .map(|pl| con_color(pl, spawn.level, t))
                .unwrap_or(t.spawn_npc);
            Style::default().fg(color)
        }
        SpawnType::Corpse => Style::default().fg(t.spawn_corpse),
        SpawnType::Unknown(_) => Style::default().fg(t.spawn_unknown),
    }
}

// ─── Spawn info lines ────────────────────────────────────────────────────────

/// Render a `SpawnInfo` as a list of styled lines (used by target panel and character screen).
pub fn spawn_info_lines(
    spawn: &SpawnInfo,
    redact: &dyn Fn(&str) -> std::borrow::Cow<str>,
    t: &Theme,
) -> Vec<Line<'static>> {
    let hp_pct = spawn.hp_pct();
    let hp_col = hp_color(hp_pct, t);
    let name = redact(&spawn.displayed_name).into_owned();
    let rawname = redact(&spawn.name).into_owned();

    vec![
        Line::from(vec![
            Span::styled(
                name,
                Style::default()
                    .fg(t.text_bright)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} Lv{}", spawn.class_str(), spawn.level),
                Style::default().fg(t.text_accent),
            ),
            Span::raw(format!("  [{}]  {}", spawn.spawn_type, spawn.stand_state)),
        ]),
        Line::from(vec![
            Span::styled("HP   ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}/{} ({:.0}%)", spawn.hp_current, spawn.hp_max, hp_pct),
                Style::default().fg(hp_col),
            ),
        ]),
        Line::from(vec![
            Span::styled("Mana ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}/{}", spawn.mana_current, spawn.mana_max),
                Style::default().fg(t.mana_color),
            ),
            Span::styled(
                format!("  End {}/{}", spawn.endurance_current, spawn.endurance_max),
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::from(vec![
            Span::styled("Pos  ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("({:.1}, {:.1}, {:.1})", spawn.y, spawn.x, spawn.z),
                Style::default().fg(t.text_server),
            ),
            Span::styled(
                format!("  Hdg {:.1}", spawn.heading),
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("ID {} ", spawn.spawn_id),
                Style::default().fg(t.text_muted),
            ),
            Span::styled(rawname, Style::default().fg(t.text_secondary)),
        ]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::dark_modern;

    #[test]
    fn con_color_red_when_much_higher() {
        let t = dark_modern();
        assert_eq!(con_color(30, 34, &t), t.con_red);
        assert_eq!(con_color(30, 40, &t), t.con_red);
    }

    #[test]
    fn con_color_yellow_when_slightly_higher() {
        let t = dark_modern();
        assert_eq!(con_color(30, 31, &t), t.con_yellow);
        assert_eq!(con_color(30, 33, &t), t.con_yellow);
    }

    #[test]
    fn con_color_white_when_same() {
        let t = dark_modern();
        assert_eq!(con_color(30, 30, &t), t.con_white);
    }

    #[test]
    fn con_color_lightcyan_when_slightly_lower() {
        let t = dark_modern();
        assert_eq!(con_color(30, 29, &t), t.con_light_blue);
        assert_eq!(con_color(30, 27, &t), t.con_light_blue);
    }

    #[test]
    fn con_color_blue_when_lower() {
        let t = dark_modern();
        assert_eq!(con_color(30, 26, &t), t.con_blue);
        assert_eq!(con_color(30, 24, &t), t.con_blue);
    }

    #[test]
    fn con_color_green_when_trivial() {
        let t = dark_modern();
        assert_eq!(con_color(30, 23, &t), t.con_green);
        assert_eq!(con_color(30, 1, &t), t.con_green);
    }
}
