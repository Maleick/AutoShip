use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::time::Duration;

use super::app::{ActivePanel, ActiveScreen, App};

/// Poll for keyboard events and update app state.
/// Returns true if an event was handled.
pub fn handle_events(app: &mut App, timeout: Duration) -> Result<bool> {
    if !event::poll(timeout)? {
        return Ok(false);
    }

    if let Event::Key(key) = event::read()? {
        // Only handle key press events, not repeat or release.
        // This prevents toggles (like privacy mode) from bouncing.
        if key.kind != KeyEventKind::Press {
            return Ok(false);
        }

        // When in search mode, capture text input
        if app.search_mode {
            match key.code {
                KeyCode::Esc => {
                    app.search_mode = false;
                    return Ok(true);
                }
                KeyCode::Enter => {
                    app.search_mode = false;
                    return Ok(true);
                }
                KeyCode::Backspace => {
                    app.spawn_filter.pop();
                    app.spawn_selected = 0;
                    return Ok(true);
                }
                KeyCode::Char(c) => {
                    app.spawn_filter.push(c);
                    app.spawn_selected = 0;
                    return Ok(true);
                }
                _ => return Ok(false),
            }
        }

        // Global keybindings
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _) => {
                app.running = false;
                return Ok(true);
            }
            // Screen switching
            (KeyCode::Char('1'), _) => {
                app.active_screen = ActiveScreen::Dashboard;
                return Ok(true);
            }
            (KeyCode::Char('2'), _) => {
                app.active_screen = ActiveScreen::Spawns;
                return Ok(true);
            }
            (KeyCode::Char('3'), _) => {
                app.active_screen = ActiveScreen::Character;
                return Ok(true);
            }
            (KeyCode::Char('4'), _) => {
                app.active_screen = ActiveScreen::Map;
                return Ok(true);
            }
            (KeyCode::Tab, _) => {
                app.toggle_panel();
                return Ok(true);
            }
            // Client switching: ] = next client, [ = previous client
            (KeyCode::Char(']'), _) => {
                app.next_client();
                return Ok(true);
            }
            (KeyCode::Char('['), _) => {
                app.prev_client();
                return Ok(true);
            }
            (KeyCode::Char('p'), _) => {
                app.toggle_privacy();
                return Ok(true);
            }
            (KeyCode::Char('/'), _) => {
                app.search_mode = true;
                app.spawn_filter.clear();
                // Switch to Spawns screen if not already there
                if app.active_screen != ActiveScreen::Spawns {
                    app.active_screen = ActiveScreen::Spawns;
                }
                return Ok(true);
            }
            (KeyCode::Char('f'), _) => {
                app.cycle_spawn_filter();
                return Ok(true);
            }
            (KeyCode::Esc, _) => {
                app.clear_filter();
                return Ok(true);
            }
            _ => {}
        }

        // Panel-specific keybindings (apply on Spawns and Character screens)
        match app.active_panel {
            ActivePanel::SpawnList => match key.code {
                KeyCode::Down | KeyCode::Char('j') => app.spawn_list_down(),
                KeyCode::Up | KeyCode::Char('k') => app.spawn_list_up(),
                KeyCode::PageDown => app.spawn_list_page_down(),
                KeyCode::PageUp => app.spawn_list_page_up(),
                KeyCode::Home => app.spawn_selected = 0,
                KeyCode::End => {
                    let max = app.filtered_spawns().len().saturating_sub(1);
                    app.spawn_selected = max;
                }
                KeyCode::Enter => app.inspect_selected_spawn(),
                _ => {}
            },
            ActivePanel::HexDump => match key.code {
                KeyCode::Down | KeyCode::Char('j') => app.hex_scroll_down(),
                KeyCode::Up | KeyCode::Char('k') => app.hex_scroll_up(),
                _ => {}
            },
        }
    }

    Ok(true)
}
