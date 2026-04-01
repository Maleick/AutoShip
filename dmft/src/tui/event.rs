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
        if key.kind != KeyEventKind::Press {
            return Ok(false);
        }

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

        if app.help_visible {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => app.help_visible = false,
                _ => {}
            }
            return Ok(true);
        }

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

        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _) => {
                app.running = false;
                return Ok(true);
            }
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
            (KeyCode::Char('1'), _) => {
                app.set_active_screen(ActiveScreen::Overview);
                return Ok(true);
            }
            (KeyCode::Char('2'), _) => {
                app.set_active_screen(ActiveScreen::Tactical);
                return Ok(true);
            }
            (KeyCode::Char('3'), _) => {
                app.set_active_screen(ActiveScreen::Navigation);
                return Ok(true);
            }
            (KeyCode::Char('4'), _) => {
                app.set_active_screen(ActiveScreen::Debug);
                return Ok(true);
            }
            (KeyCode::Tab, _) => {
                app.toggle_panel();
                return Ok(true);
            }
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
                app.set_active_screen(ActiveScreen::Tactical);
                app.active_panel = ActivePanel::TacticalSpawns;
                return Ok(true);
            }
            (KeyCode::Char('f'), _) => {
                app.cycle_spawn_filter();
                return Ok(true);
            }
            (KeyCode::Char('g'), _) if app.active_screen == ActiveScreen::Overview => {
                app.toggle_groups_visibility();
                return Ok(true);
            }
            (KeyCode::Char('v'), _) if app.active_screen == ActiveScreen::Overview => {
                app.toggle_filters_visibility();
                return Ok(true);
            }
            (KeyCode::Char('z'), _) => {
                app.toggle_focused_section();
                return Ok(true);
            }
            (KeyCode::Char('T'), _) => {
                app.cycle_theme();
                return Ok(true);
            }
            (KeyCode::Char('m' | 'M'), _) if app.active_screen == ActiveScreen::Tactical => {
                app.toggle_tactical_map_maximized();
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
            (KeyCode::F(n), _) if (1..=9).contains(&n) => {
                let idx = (n - 1) as usize;
                if let Some(cmd) = app.cmd_state.get_favorite(idx).map(|s| s.to_string()) {
                    app.cmd_state.command_buffer = cmd;
                    app.execute_command(orchestrator);
                    app.cmd_state.command_buffer.clear();
                } else {
                    app.status_message = format!(
                        "F{}: no favorite assigned (use commands to build frequency)",
                        n
                    );
                }
                return Ok(true);
            }
            _ => {}
        }

        if matches!(
            app.active_screen,
            ActiveScreen::Overview
                | ActiveScreen::Tactical
                | ActiveScreen::Navigation
                | ActiveScreen::Debug
        ) {
            match key.code {
                KeyCode::Char('r') => {
                    if let Some(last) = app.cmd_state.command_history.last().cloned() {
                        app.cmd_state.command_buffer = last;
                        app.execute_command(orchestrator);
                        app.cmd_state.command_buffer.clear();
                    } else {
                        app.status_message = "No command history to repeat".into();
                    }
                    return Ok(true);
                }
                KeyCode::Char('e') => {
                    app.cmd_state.command_buffer = "engage".into();
                    app.execute_command(orchestrator);
                    app.cmd_state.command_buffer.clear();
                    return Ok(true);
                }
                KeyCode::Char('d') => {
                    app.cmd_state.command_buffer = "disengage".into();
                    app.execute_command(orchestrator);
                    app.cmd_state.command_buffer.clear();
                    return Ok(true);
                }
                KeyCode::Char('l') => {
                    app.cmd_state.command_buffer = "loot".into();
                    app.execute_command(orchestrator);
                    app.cmd_state.command_buffer.clear();
                    return Ok(true);
                }
                _ => {}
            }
        }

        if app.active_screen == ActiveScreen::Tactical {
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

        match app.active_panel {
            ActivePanel::OverviewRoster => match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    app.next_client();
                    return Ok(true);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    app.prev_client();
                    return Ok(true);
                }
                KeyCode::Enter => {
                    app.expand_selected_character();
                    return Ok(true);
                }
                _ => {}
            },
            ActivePanel::TacticalMap => {
                if key.code == KeyCode::Enter {
                    app.toggle_tactical_map_maximized();
                    return Ok(true);
                }
            }
            ActivePanel::TacticalNavigation => match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    app.next_client();
                    return Ok(true);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    app.prev_client();
                    return Ok(true);
                }
                KeyCode::Enter => {
                    if app.active_screen == ActiveScreen::Navigation {
                        app.set_active_screen(ActiveScreen::Tactical);
                        app.active_panel = ActivePanel::TacticalNavigation;
                        app.status_message = String::from("Navigation: returned to Map screen");
                    } else {
                        app.set_active_screen(ActiveScreen::Navigation);
                        app.status_message = String::from("Navigation: full status window opened");
                    }
                    return Ok(true);
                }
                _ => {}
            },
            ActivePanel::TacticalSpawns | ActivePanel::DebugSpawns => match key.code {
                KeyCode::Down | KeyCode::Char('j') => app.spawn_list_down(),
                KeyCode::Up | KeyCode::Char('k') => app.spawn_list_up(),
                KeyCode::PageDown => app.spawn_list_page_down(),
                KeyCode::PageUp => app.spawn_list_page_up(),
                KeyCode::Home => app.spawns_state.table_state.select(Some(0)),
                KeyCode::End => {
                    let max = app.filtered_spawns().len().saturating_sub(1);
                    app.spawns_state.table_state.select(Some(max));
                }
                KeyCode::Enter => app.debug_selected_spawn(),
                _ => {}
            },
            ActivePanel::DebugHexDump => match key.code {
                KeyCode::Down => app.hex_scroll_down(),
                KeyCode::Up => app.hex_scroll_up(),
                _ => {}
            },
            _ => {}
        }
    }

    Ok(true)
}
