use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::time::Duration;

use super::app::{ActivePanel, App};

/// Poll for keyboard events and update app state.
/// Returns true if an event was handled.
pub fn handle_events(app: &mut App, timeout: Duration) -> Result<bool> {
    if !event::poll(timeout)? {
        return Ok(false);
    }

    if let Event::Key(key) = event::read()? {
        // Global keybindings
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _) => {
                app.running = false;
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
            (KeyCode::Char('/'), _) if app.active_panel == ActivePanel::SpawnList => {
                // TODO: enter filter mode — for now just clear filter
                app.clear_filter();
                return Ok(true);
            }
            (KeyCode::Esc, _) => {
                app.clear_filter();
                return Ok(true);
            }
            _ => {}
        }

        // Panel-specific keybindings
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
