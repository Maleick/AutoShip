use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::time::Duration;

use super::app::{ActivePanel, ActiveScreen, App};
use crate::orchestrator::Orchestrator;

/// Poll for keyboard events and update app state.
/// Returns true if an event was handled.
pub fn handle_events(
    app: &mut App,
    timeout: Duration,
    orchestrator: &mut Orchestrator,
) -> Result<bool> {
    if !event::poll(timeout)? {
        return Ok(false);
    }

    if let Event::Key(key) = event::read()? {
        // Only handle key press events, not repeat or release.
        // This prevents toggles (like privacy mode) from bouncing.
        if key.kind != KeyEventKind::Press {
            return Ok(false);
        }

        // Command mode (: prefix) — checked first
        if app.cmd_state.command_mode {
            match key.code {
                KeyCode::Esc => {
                    app.cmd_state.command_mode = false;
                    app.cmd_state.command_buffer.clear();
                    return Ok(true);
                }
                KeyCode::Enter => {
                    app.cmd_state.command_mode = false;
                    app.cmd_state.command_history_idx = None;
                    app.execute_command(orchestrator);
                    app.cmd_state.command_buffer.clear();
                    return Ok(true);
                }
                KeyCode::Backspace => {
                    app.cmd_state.command_buffer.pop();
                    return Ok(true);
                }
                KeyCode::Up => {
                    if !app.cmd_state.command_history.is_empty() {
                        let idx = match app.cmd_state.command_history_idx {
                            Some(i) => i.saturating_sub(1),
                            None => app.cmd_state.command_history.len() - 1,
                        };
                        app.cmd_state.command_history_idx = Some(idx);
                        app.cmd_state.command_buffer = app.cmd_state.command_history[idx].clone();
                    }
                    return Ok(true);
                }
                KeyCode::Down => {
                    if let Some(idx) = app.cmd_state.command_history_idx {
                        if idx + 1 < app.cmd_state.command_history.len() {
                            let next = idx + 1;
                            app.cmd_state.command_history_idx = Some(next);
                            app.cmd_state.command_buffer =
                                app.cmd_state.command_history[next].clone();
                        } else {
                            app.cmd_state.command_history_idx = None;
                            app.cmd_state.command_buffer.clear();
                        }
                    }
                    return Ok(true);
                }
                KeyCode::Tab => {
                    app.complete_command();
                    return Ok(true);
                }
                KeyCode::Char(c) => {
                    app.cmd_state.command_buffer.push(c);
                    return Ok(true);
                }
                _ => return Ok(false),
            }
        }

        // Help overlay — dismiss with ? or Esc
        if app.help_visible {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => {
                    app.help_visible = false;
                }
                _ => {}
            }
            return Ok(true);
        }

        // When in search mode, capture text input
        if app.spawns_state.search_mode {
            match key.code {
                KeyCode::Esc => {
                    app.spawns_state.search_mode = false;
                    return Ok(true);
                }
                KeyCode::Enter => {
                    app.spawns_state.search_mode = false;
                    return Ok(true);
                }
                KeyCode::Backspace => {
                    app.spawns_state.spawn_filter.pop();
                    app.spawns_state.table_state.select(Some(0));
                    return Ok(true);
                }
                KeyCode::Char(c) => {
                    app.spawns_state.spawn_filter.push(c);
                    app.spawns_state.table_state.select(Some(0));
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
            // Group focus: Shift+1-6 = focus group, Shift+0 or G+Esc = aggregate
            (KeyCode::Char('!'), _) => {
                app.set_active_group(Some(0));
                return Ok(true);
            }
            (KeyCode::Char('@'), _) => {
                app.set_active_group(Some(1));
                return Ok(true);
            }
            (KeyCode::Char('#'), _) => {
                app.set_active_group(Some(2));
                return Ok(true);
            }
            (KeyCode::Char('$'), _) => {
                app.set_active_group(Some(3));
                return Ok(true);
            }
            (KeyCode::Char('%'), _) => {
                app.set_active_group(Some(4));
                return Ok(true);
            }
            (KeyCode::Char('^'), _) => {
                app.set_active_group(Some(5));
                return Ok(true);
            }
            (KeyCode::Char(')'), _) => {
                app.set_active_group(None);
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
            (KeyCode::Char('5'), _) => {
                app.active_screen = ActiveScreen::Groups;
                return Ok(true);
            }
            (KeyCode::Char('6'), _) => {
                app.active_screen = ActiveScreen::Navigation;
                return Ok(true);
            }
            (KeyCode::Tab, _) if app.active_screen == ActiveScreen::Character => {
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
            (KeyCode::Char('?'), _) => {
                app.help_visible = !app.help_visible;
                return Ok(true);
            }
            (KeyCode::Char(':'), _) => {
                app.cmd_state.command_mode = true;
                app.cmd_state.command_buffer.clear();
                app.cmd_state.command_history_idx = None;
                return Ok(true);
            }
            (KeyCode::Char('/'), _) => {
                app.spawns_state.search_mode = true;
                app.spawns_state.spawn_filter.clear();
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
            (KeyCode::Char('T'), _) => {
                app.cycle_theme();
                return Ok(true);
            }
            (KeyCode::Esc, _) => {
                if app.active_group.is_some() {
                    app.set_active_group(None);
                } else {
                    app.clear_filter();
                }
                return Ok(true);
            }
            // F1-F9: Execute favorite commands (most frequently used)
            (KeyCode::F(n), _) if (1..=9).contains(&n) => {
                let idx = (n - 1) as usize;
                if let Some(cmd) = app.cmd_state.get_favorite(idx).map(|s| s.to_string()) {
                    app.cmd_state.command_buffer = cmd;
                    app.execute_command(orchestrator);
                    app.cmd_state.command_buffer.clear();
                } else {
                    app.status_message = format!("F{}: no favorite assigned (use commands to build frequency)", n);
                }
                return Ok(true);
            }
            _ => {}
        }

        // Map-screen keybindings: +/- adjust Z-depth filter
        if app.active_screen == ActiveScreen::Map {
            match key.code {
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    app.map_state.increase_z_filter();
                    return Ok(true);
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    app.map_state.decrease_z_filter();
                    return Ok(true);
                }
                _ => {}
            }
        }

        // Navigation screen keybindings: arrows navigate the character list
        if app.active_screen == ActiveScreen::Navigation {
            match key.code {
                KeyCode::Down => {
                    let max = app.visible_clients().len().saturating_sub(1);
                    if app.nav_state.nav_selected < max {
                        app.nav_state.nav_selected += 1;
                    }
                }
                KeyCode::Up => {
                    app.nav_state.nav_selected = app.nav_state.nav_selected.saturating_sub(1);
                }
                _ => {}
            }
        }

        // Panel-specific keybindings (only on Spawns and Character screens)
        if app.active_screen == ActiveScreen::Spawns || app.active_screen == ActiveScreen::Character
        {
            match app.active_panel {
                ActivePanel::SpawnList => match key.code {
                    KeyCode::Down => app.spawn_list_down(),
                    KeyCode::Up => app.spawn_list_up(),
                    KeyCode::PageDown => app.spawn_list_page_down(),
                    KeyCode::PageUp => app.spawn_list_page_up(),
                    KeyCode::Home => app.spawns_state.table_state.select(Some(0)),
                    KeyCode::End => {
                        let max = app.filtered_spawns().len().saturating_sub(1);
                        app.spawns_state.table_state.select(Some(max));
                    }
                    KeyCode::Enter => app.inspect_selected_spawn(),
                    _ => {}
                },
                ActivePanel::HexDump => match key.code {
                    KeyCode::Down => app.hex_scroll_down(),
                    KeyCode::Up => app.hex_scroll_up(),
                    _ => {}
                },
            }
        }
    }

    Ok(true)
}
