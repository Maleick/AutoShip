//! Per-character personality and LLM model selector panel.
//!
//! Renders a compact info row showing the active personality preset and mapped
//! model for the currently selected character.  When the selector popup is
//! open (`PersonalitySelectorState::open == true`) a list overlay lets the
//! operator cycle through presets and apply them without restarting.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::tui::{
    app::{App, PersonalitySelectorState},
    theme::Theme,
    ui::widgets::panel,
};

// ─── Public render entry points ───────────────────────────────────────────────

/// Draw the personality/model info strip for the currently selected character.
///
/// Shows the active personality preset label and mapped model name.
/// If no character is selected the panel renders a short placeholder.
///
/// Call this from the overview sidebar or character detail area.
pub fn draw_personality_strip(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let border_style = theme.border_dim;
    let blk = panel(
        Line::from(vec![
            Span::styled(" Soul ", Style::default().fg(theme.text_accent)),
            Span::styled("Personality", Style::default().fg(theme.text_secondary)),
            Span::styled(" ", Style::default()),
        ]),
        border_style,
        theme,
    );

    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    // Determine the active character name
    let char_name: Option<String> = app
        .clients
        .get(app.selected_client)
        .map(|c| {
            if !c.character_name.is_empty() {
                c.character_name.clone()
            } else {
                c.local_player
                    .as_ref()
                    .map(|p| p.displayed_name.clone())
                    .unwrap_or_default()
            }
        })
        .filter(|s| !s.is_empty());

    let lines = build_info_lines(&app.personality_selector, char_name.as_deref(), app, theme);
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Draw the personality selector popup overlay.
///
/// Shows a scrollable list of personality presets. The currently selected
/// preset is highlighted.  Press Enter to apply, Escape to close.
pub fn draw_personality_popup(frame: &mut Frame, app: &App, theme: &Theme) {
    if !app.personality_selector.open {
        return;
    }

    let area = frame.area();
    let popup_area = centered_popup(area, 40, 20);

    // Clear behind the popup
    frame.render_widget(Clear, popup_area);

    let char_name: String = app
        .clients
        .get(app.selected_client)
        .map(|c| {
            if !c.character_name.is_empty() {
                c.character_name.clone()
            } else {
                c.local_player
                    .as_ref()
                    .map(|p| p.displayed_name.clone())
                    .unwrap_or_else(|| "Unknown".into())
            }
        })
        .unwrap_or_else(|| "Unknown".into());

    let title = Line::from(vec![
        Span::styled(" Personality — ", Style::default().fg(theme.text_accent)),
        Span::styled(char_name.clone(), Style::default().fg(theme.text_bright)),
        Span::styled(" ", Style::default()),
    ]);

    let blk = Block::default()
        .borders(Borders::ALL)
        .border_type(theme.border_type)
        .title(title)
        .border_style(theme.border_active);

    let inner = blk.inner(popup_area);
    frame.render_widget(blk, popup_area);

    // Split inner: preset list top, model list bottom, hint bar at very bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(8),
            Constraint::Length(1),
        ])
        .split(inner);

    // Personality preset list
    let preset_items: Vec<ListItem> = PersonalitySelectorState::PRESETS
        .iter()
        .enumerate()
        .map(|(i, &name)| {
            let active = app.personality_selector.active_personality(&char_name) == name;
            let selected = i == app.personality_selector.preset_cursor;
            let style = if selected && active {
                Style::default()
                    .fg(theme.text_bright)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else if selected {
                Style::default().fg(theme.text_accent).bg(Color::DarkGray)
            } else if active {
                Style::default()
                    .fg(theme.text_highlight)
                    .add_modifier(Modifier::ITALIC)
            } else {
                Style::default().fg(theme.text_normal)
            };
            let marker = if active { "▶ " } else { "  " };
            ListItem::new(Line::from(Span::styled(
                format!("{marker}{name}"),
                style,
            )))
        })
        .collect();

    let preset_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(theme.border_dim)
        .title(Span::styled(" Preset ", Style::default().fg(theme.text_secondary)));

    let preset_list =
        List::new(preset_items).block(preset_block).highlight_symbol("▶ ");

    let mut preset_state = ListState::default();
    preset_state.select(Some(app.personality_selector.preset_cursor));
    frame.render_stateful_widget(preset_list, chunks[0], &mut preset_state);

    // Model list
    let active_model = app.personality_selector.active_model(&char_name);
    let coord_model = app
        .soul_coordinator
        .as_ref()
        .map(|c| c.active_model().to_owned())
        .unwrap_or_else(|| "none".into());
    let effective_model = if active_model.is_empty() {
        coord_model.as_str()
    } else {
        active_model
    };

    let model_items: Vec<ListItem> = PersonalitySelectorState::MODELS
        .iter()
        .map(|&m| {
            let active = m == effective_model;
            let style = if active {
                Style::default()
                    .fg(theme.text_highlight)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_normal)
            };
            let marker = if active { "▶ " } else { "  " };
            ListItem::new(Line::from(Span::styled(format!("{marker}{m}"), style)))
        })
        .collect();

    let model_block = Block::default()
        .borders(Borders::NONE)
        .title(Span::styled(" Model ", Style::default().fg(theme.text_secondary)));

    let model_list = List::new(model_items).block(model_block);
    frame.render_widget(model_list, chunks[1]);

    // Hint bar
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("↑↓ ", Style::default().fg(theme.text_accent)),
        Span::styled("cycle  ", Style::default().fg(theme.text_muted)),
        Span::styled("Enter ", Style::default().fg(theme.text_accent)),
        Span::styled("apply  ", Style::default().fg(theme.text_muted)),
        Span::styled("Esc ", Style::default().fg(theme.text_accent)),
        Span::styled("close", Style::default().fg(theme.text_muted)),
    ]));
    frame.render_widget(hint, chunks[2]);
}

// ─── Private helpers ──────────────────────────────────────────────────────────

fn build_info_lines<'a>(
    state: &'a PersonalitySelectorState,
    char_name: Option<&'a str>,
    app: &'a App,
    theme: &'a Theme,
) -> Vec<Line<'a>> {
    let Some(name) = char_name else {
        return vec![Line::from(Span::styled(
            "No character selected",
            Style::default().fg(theme.text_muted),
        ))];
    };

    let personality = state.active_personality(name);
    let model_override = state.active_model(name);
    let coord_model = app
        .soul_coordinator
        .as_ref()
        .map(|c| c.active_model().to_owned())
        .unwrap_or_else(|| "none".into());
    let effective_model = if model_override.is_empty() {
        coord_model.as_str()
    } else {
        model_override
    };

    let mood_label = app
        .soul_coordinator
        .as_ref()
        .and_then(|c| {
            // ClientId is a u32 alias matching the client's pid.
            app.clients
                .get(app.selected_client)
                .and_then(|cl| c.mood(cl.pid))
                .map(|m| format!("{m:?}"))
        })
        .unwrap_or_else(|| "—".into());

    vec![
        Line::from(vec![
            Span::styled("Char     ", Style::default().fg(theme.text_secondary)),
            Span::styled(name, Style::default().fg(theme.text_bright)),
        ]),
        Line::from(vec![
            Span::styled("Mood     ", Style::default().fg(theme.text_secondary)),
            Span::styled(mood_label, Style::default().fg(theme.text_normal)),
        ]),
        Line::from(vec![
            Span::styled("Preset   ", Style::default().fg(theme.text_secondary)),
            Span::styled(
                personality,
                Style::default()
                    .fg(theme.text_highlight)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Model    ", Style::default().fg(theme.text_secondary)),
            Span::styled(
                effective_model,
                Style::default().fg(theme.text_accent),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  [P] open selector",
                Style::default().fg(theme.text_muted),
            ),
        ]),
    ]
}

/// Create a centered popup rect of fixed width×height inside `area`.
fn centered_popup(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personality_selector_state_presets_non_empty() {
        assert!(!PersonalitySelectorState::PRESETS.is_empty());
    }

    #[test]
    fn personality_selector_state_models_non_empty() {
        assert!(!PersonalitySelectorState::MODELS.is_empty());
    }

    #[test]
    fn cursor_wraps_forward() {
        let mut state = PersonalitySelectorState::default();
        let n = PersonalitySelectorState::PRESETS.len();
        for _ in 0..n {
            state.cursor_next();
        }
        assert_eq!(state.preset_cursor, 0);
    }

    #[test]
    fn cursor_wraps_backward() {
        let mut state = PersonalitySelectorState::default();
        state.cursor_prev();
        assert_eq!(
            state.preset_cursor,
            PersonalitySelectorState::PRESETS.len().saturating_sub(1)
        );
    }

    #[test]
    fn set_personality_persists() {
        let mut state = PersonalitySelectorState::default();
        state.set_personality("Grimjaw", "Aggressive");
        assert_eq!(state.active_personality("Grimjaw"), "Aggressive");
    }

    #[test]
    fn active_personality_default_for_unknown() {
        let state = PersonalitySelectorState::default();
        assert_eq!(state.active_personality("Nobody"), "Default");
    }

    #[test]
    fn set_model_override_persists() {
        let mut state = PersonalitySelectorState::default();
        state.set_model_override("Grimjaw", "llama3:8b");
        assert_eq!(state.active_model("Grimjaw"), "llama3:8b");
    }

    #[test]
    fn active_model_empty_for_unknown() {
        let state = PersonalitySelectorState::default();
        assert_eq!(state.active_model("Nobody"), "");
    }
}
