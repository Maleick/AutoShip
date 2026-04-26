use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

use super::app::{ActivePanel, ActiveScreen, App, HelpFocus, MapViewportMode, ToastLevel};
use crate::{
    orchestrator::Orchestrator,
    session_replay,
    tui::{state::MapFilterKind, ui::ch_chain::ChPanelFocus},
};

fn toggle_tactical_map_layer(app: &mut App, layer: u8) {
    if layer == 4 {
        app.toggle_tactical_navmesh_overlay();
        return;
    }
    let status = app.map_state.toggle_layer(layer);
    app.status_message = status.to_string();
    app.active_screen = ActiveScreen::Tactical;
    app.active_panel = ActivePanel::TacticalMap;
}

fn toggle_tactical_map_filter(app: &mut App, kind: MapFilterKind) {
    app.status_message = app.map_state.toggle_filter(kind);
    app.active_screen = ActiveScreen::Tactical;
    app.active_panel = ActivePanel::TacticalMap;
}

fn handle_tactical_map_global_shortcut(app: &mut App, key: KeyEvent) -> bool {
    if app.active_screen != ActiveScreen::Tactical || !key.modifiers.contains(KeyModifiers::ALT) {
        return false;
    }

    match key.code {
        KeyCode::Char('1' | '2' | '3' | '4' | '5' | '6' | '7') => {
            let layer = match key.code {
                KeyCode::Char('1') => 1,
                KeyCode::Char('2') => 2,
                KeyCode::Char('3') => 3,
                KeyCode::Char('4') => 4,
                KeyCode::Char('5') => 5,
                KeyCode::Char('6') => 6,
                KeyCode::Char('7') => 7,
                _ => unreachable!(),
            };
            toggle_tactical_map_layer(app, layer);
            true
        }
        KeyCode::Char('n' | 'N') => {
            toggle_tactical_map_filter(app, MapFilterKind::Npc);
            true
        }
        KeyCode::Char('p' | 'P') => {
            toggle_tactical_map_filter(app, MapFilterKind::Pc);
            true
        }
        KeyCode::Char('c' | 'C') => {
            toggle_tactical_map_filter(app, MapFilterKind::Corpse);
            true
        }
        KeyCode::Char('g' | 'G') => {
            toggle_tactical_map_filter(app, MapFilterKind::Ground);
            true
        }
        KeyCode::Char('t' | 'T') => {
            toggle_tactical_map_filter(app, MapFilterKind::Pet);
            true
        }
        KeyCode::Char('r' | 'R') => {
            toggle_tactical_map_filter(app, MapFilterKind::Named);
            true
        }
        KeyCode::Char('u' | 'U') => {
            toggle_tactical_map_filter(app, MapFilterKind::Untargetable);
            true
        }
        _ => false,
    }
}

fn handle_tactical_map_panel_toggle(app: &mut App, key: KeyCode) -> bool {
    match key {
        // Layer toggles — Mac-friendly alternatives to Alt+1-7
        KeyCode::Char('g') => toggle_tactical_map_layer(app, 1),
        KeyCode::Char('s') => toggle_tactical_map_layer(app, 2),
        KeyCode::Char('w') => toggle_tactical_map_layer(app, 3),
        KeyCode::Char('x') | KeyCode::Char('X') => {
            app.toggle_tactical_navmesh_overlay();
            return true;
        }
        KeyCode::Char('l') => toggle_tactical_map_layer(app, 5),
        KeyCode::Char('a') => toggle_tactical_map_layer(app, 6),
        KeyCode::Char('e') => toggle_tactical_map_layer(app, 7),
        KeyCode::Char('N') => toggle_tactical_map_filter(app, MapFilterKind::Npc),
        KeyCode::Char('P') => toggle_tactical_map_filter(app, MapFilterKind::Pc),
        KeyCode::Char('C') => toggle_tactical_map_filter(app, MapFilterKind::Corpse),
        KeyCode::Char('G') => toggle_tactical_map_filter(app, MapFilterKind::Ground),
        KeyCode::Char('T') => toggle_tactical_map_filter(app, MapFilterKind::Pet),
        KeyCode::Char('R') => toggle_tactical_map_filter(app, MapFilterKind::Named),
        KeyCode::Char('U') => toggle_tactical_map_filter(app, MapFilterKind::Untargetable),
        _ => return false,
    }

    true
}

fn handle_debug_explorer_panel_key(app: &mut App, key: KeyCode) -> bool {
    if app.explorer_state.search_mode {
        match key {
            KeyCode::Esc => app.explorer_state.search_mode = false,
            KeyCode::Enter => {
                app.explorer_state.search_mode = false;
                app.explorer_state.apply_filter();
            }
            KeyCode::Backspace => {
                app.explorer_state.search_filter.pop();
                app.explorer_state.apply_filter();
            }
            KeyCode::Char(ch) => {
                app.explorer_state.search_filter.push(ch);
                app.explorer_state.apply_filter();
            }
            _ => return false,
        }
        return true;
    }

    match key {
        KeyCode::Down | KeyCode::Char('j') => app.explorer_state.select_next(),
        KeyCode::Up | KeyCode::Char('k') => app.explorer_state.select_prev(),
        KeyCode::Enter => app.explorer_select_function(),
        KeyCode::Char('c') => {
            app.explorer_state.category_filter = app.explorer_state.category_filter.next();
            app.explorer_state.apply_filter();
        }
        KeyCode::Char('/') => app.explorer_state.search_mode = true,
        _ => return false,
    }

    true
}

fn handle_packet_monitor_log_key(app: &mut App, key: KeyCode) -> bool {
    match key {
        KeyCode::Down | KeyCode::Char('j') => app.packet_monitor_state.select_next(),
        KeyCode::Up | KeyCode::Char('k') => app.packet_monitor_state.select_prev(),
        KeyCode::PageDown => app.packet_monitor_state.scroll_down(),
        KeyCode::PageUp => app.packet_monitor_state.scroll_up(),
        KeyCode::Home => {
            app.packet_monitor_state.auto_scroll = false;
            app.packet_monitor_state.table_state.select(Some(0));
        }
        KeyCode::End => {
            app.packet_monitor_state.auto_scroll = true;
            app.packet_monitor_state.scroll_offset = 0;
            let max = app
                .packet_monitor_state
                .filtered_packets()
                .len()
                .saturating_sub(1);
            app.packet_monitor_state.table_state.select(Some(max));
        }
        KeyCode::Char(' ') | KeyCode::Char('p' | 'P') => {
            app.packet_monitor_state.toggle_pause();
            app.status_message = if app.packet_monitor_state.paused {
                String::from("Packet monitor paused")
            } else {
                String::from("Packet monitor resumed")
            };
        }
        KeyCode::Char('c') => {
            app.packet_monitor_state.clear();
            app.status_message = String::from("Packet monitor cleared");
        }
        _ => return false,
    }

    true
}

fn handle_tactical_map_focused_shortcut(app: &mut App, key: KeyEvent) -> bool {
    if app.active_screen != ActiveScreen::Tactical || app.active_panel != ActivePanel::TacticalMap {
        return false;
    }

    if key.modifiers.contains(KeyModifiers::ALT) {
        return false;
    }

    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::Char('!' | '1') => {
                if let Some(preset) = app.map_state.select_preset_by_index(0) {
                    app.status_message = format!("Map preset: {}", preset.label());
                }
                return true;
            }
            KeyCode::Char('@' | '2') => {
                if let Some(preset) = app.map_state.select_preset_by_index(1) {
                    app.status_message = format!("Map preset: {}", preset.label());
                }
                return true;
            }
            KeyCode::Char('#' | '3') => {
                if let Some(preset) = app.map_state.select_preset_by_index(2) {
                    app.status_message = format!("Map preset: {}", preset.label());
                }
                return true;
            }
            KeyCode::Char('$' | '4') => {
                if let Some(preset) = app.map_state.select_preset_by_index(3) {
                    app.status_message = format!("Map preset: {}", preset.label());
                }
                return true;
            }
            KeyCode::Char('%' | '5') => {
                if let Some(preset) = app.map_state.select_preset_by_index(4) {
                    app.status_message = format!("Map preset: {}", preset.label());
                }
                return true;
            }
            _ => {}
        }
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('a' | 'A') => {
                let _ = app.set_tactical_map_view_mode(MapViewportMode::Auto);
                return true;
            }
            KeyCode::Char('l' | 'L') => {
                let _ = app.set_tactical_map_view_mode(MapViewportMode::Local);
                return true;
            }
            KeyCode::Char('g' | 'G') => {
                let _ = app.set_tactical_map_view_mode(MapViewportMode::Global);
                return true;
            }
            KeyCode::Char('i' | 'I') => {
                app.show_tactical_zone_info();
                return true;
            }
            _ => return false,
        }
    }

    match key.code {
        KeyCode::Char('g') => toggle_tactical_map_layer(app, 1),
        KeyCode::Char('s') => toggle_tactical_map_layer(app, 2),
        KeyCode::Char('I') => {
            app.show_tactical_zone_info();
            return true;
        }
        KeyCode::Char('w') | KeyCode::Char('W') => toggle_tactical_map_layer(app, 3),
        KeyCode::Char('x') => {
            app.toggle_tactical_navmesh_overlay();
            return true;
        }
        KeyCode::Char('l') => toggle_tactical_map_layer(app, 5),
        KeyCode::Char('a') => toggle_tactical_map_layer(app, 6),
        KeyCode::Char('n') => toggle_tactical_map_filter(app, MapFilterKind::Npc),
        KeyCode::Char('p') => toggle_tactical_map_filter(app, MapFilterKind::Pc),
        KeyCode::Char('c') => toggle_tactical_map_filter(app, MapFilterKind::Corpse),
        KeyCode::Char('t') => toggle_tactical_map_filter(app, MapFilterKind::Pet),
        KeyCode::Char('r') => toggle_tactical_map_filter(app, MapFilterKind::Named),
        KeyCode::Char('u') => toggle_tactical_map_filter(app, MapFilterKind::Untargetable),
        KeyCode::Char('N') => {
            app.toggle_tactical_navmesh_overlay();
            return true;
        }
        KeyCode::Home => {
            app.center_tactical_map_on_player();
            return true;
        }
        KeyCode::End => {
            app.fit_tactical_map_zone();
            return true;
        }
        KeyCode::Char('v') => {
            app.cycle_tactical_map_view();
            return true;
        }
        KeyCode::Char('[') => {
            let prev = app.map_state.prev_preset();
            app.status_message = format!("Map preset: {}", prev.label());
            return true;
        }
        KeyCode::Char(']') => {
            let next = app.map_state.next_preset();
            app.status_message = format!("Map preset: {}", next.label());
            return true;
        }
        _ => return false,
    }

    true
}

fn handle_metrics_dashboard_shortcut(app: &mut App, key: KeyEvent) -> bool {
    if app.active_screen != ActiveScreen::Metrics {
        return false;
    }

    match (key.code, key.modifiers) {
        (KeyCode::Tab, _) => app.metrics_next_tab(),
        (KeyCode::BackTab, _) => app.metrics_prev_tab(),
        (KeyCode::Right, _) | (KeyCode::Char('l'), _) => app.metrics_next_tab(),
        (KeyCode::Left, _) | (KeyCode::Char('h'), _) => app.metrics_prev_tab(),
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => app.metrics_select_next(),
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => app.metrics_select_prev(),
        (KeyCode::Enter, _) => app.metrics_toggle_detail(),
        (KeyCode::Char('s' | 'S'), _) => app.metrics_toggle_sort(),
        (KeyCode::Char('f' | 'F'), _) => app.metrics_toggle_scope(),
        (KeyCode::Char(' '), _) => app.metrics_toggle_selected_metric(),
        _ => return false,
    }

    true
}

fn execute_dashboard_command(app: &mut App, orchestrator: &mut Orchestrator, command: &str) {
    let started_at = Instant::now();
    let session_id = app.routing_scope.label();
    app.cmd_state.command_buffer = command.to_string();
    app.execute_command(orchestrator);
    app.cmd_state.command_buffer.clear();
    app.orchestrator_state
        .push_command(super::ui::orchestrator_panel::RelayCommandEntry {
            session_id,
            command: command.to_string(),
            success: true,
            latency_ms: started_at.elapsed().as_millis().max(1) as u64,
        });
}

fn handle_orchestrator_dashboard_shortcut(
    app: &mut App,
    orchestrator: &mut Orchestrator,
    key: KeyEvent,
) -> bool {
    if app.active_screen != ActiveScreen::Orchestrator {
        return false;
    }

    match key.code {
        KeyCode::Right | KeyCode::Char('l') => {
            app.orchestrator_state.next_tab();
            app.status_message = format!(
                "Dashboard tab: {}",
                app.orchestrator_state.active_tab.label()
            );
            true
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.orchestrator_state.prev_tab();
            app.status_message = format!(
                "Dashboard tab: {}",
                app.orchestrator_state.active_tab.label()
            );
            true
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.next_client();
            true
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.prev_client();
            true
        }
        KeyCode::Char(' ') => {
            app.automation_paused = !app.automation_paused;
            app.status_message = if app.automation_paused {
                String::from("Automation PAUSED")
            } else {
                String::from("Automation RESUMED")
            };
            true
        }
        KeyCode::Char('e') | KeyCode::Char('E') => {
            execute_dashboard_command(app, orchestrator, "engage");
            true
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            execute_dashboard_command(app, orchestrator, "disengage");
            true
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            execute_dashboard_command(app, orchestrator, "camp status");
            true
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            execute_dashboard_command(app, orchestrator, "nav ui");
            true
        }
        KeyCode::Char('x') | KeyCode::Delete => {
            if let Some(client) = app.active_client() {
                let pid = client.pid;
                let name = app.client_command_target(client);
                orchestrator.eject_client(pid);
                app.status_message = format!("Terminated {name} (PID {pid})");
            } else {
                app.status_message = String::from("No active client to terminate");
            }
            true
        }
        _ => false,
    }
}

fn open_help_search_panel(app: &mut App) {
    app.help_visible = false;
    app.help_focus = None;
    app.help_panel.close();
    app.help_search_visible = true;
    app.spawns_state.search_mode = false;
    app.help_search_state.clear_query();
    app.status_message =
        String::from("Help search: type to filter, q closes, :help keyboard lists shortcuts");
}

fn close_help_search_panel(app: &mut App) {
    app.help_search_visible = false;
    app.help_search_state.expanded = false;
    app.status_message = String::from("Help search closed");
}

fn handle_help_search_panel(app: &mut App, key: KeyEvent) -> bool {
    if !app.help_search_visible {
        return false;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q' | 'Q') => close_help_search_panel(app),
        KeyCode::Tab => app.help_search_state.next_tab(),
        KeyCode::BackTab => app.help_search_state.prev_tab(),
        KeyCode::Enter => {
            if !crate::tui::ui::help::filtered_entries(&app.help_search_state).is_empty() {
                app.help_search_state.toggle_expanded();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => app.help_search_state.select_prev(),
        KeyCode::Down | KeyCode::Char('j') => {
            let max = crate::tui::ui::help::filtered_entries(&app.help_search_state).len();
            app.help_search_state.select_next(max);
            app.help_search_state.ensure_visible(8);
        }
        KeyCode::PageUp => {
            for _ in 0..5 {
                app.help_search_state.select_prev();
            }
        }
        KeyCode::PageDown => {
            let max = crate::tui::ui::help::filtered_entries(&app.help_search_state).len();
            for _ in 0..5 {
                app.help_search_state.select_next(max);
            }
            app.help_search_state.ensure_visible(8);
        }
        KeyCode::Home => {
            app.help_search_state.selected = 0;
            app.help_search_state.scroll = 0;
            app.help_search_state.expanded = false;
        }
        KeyCode::End => {
            let max = crate::tui::ui::help::filtered_entries(&app.help_search_state).len();
            app.help_search_state.selected = max.saturating_sub(1);
            app.help_search_state.ensure_visible(8);
            app.help_search_state.expanded = false;
        }
        KeyCode::Backspace => app.help_search_state.pop_char(),
        KeyCode::Char('/')
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.help_search_state.clear_query();
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.help_search_state.push_char(c);
        }
        _ => {}
    }

    true
}

/// Poll for keyboard events and update app state.
/// Returns true if an event was handled.
///
/// # Errors
///
/// Returns an error if the operation fails.
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

        session_replay::record_operator_keypress(key);

        if app.cmd_state.command_mode {
            match key.code {
                KeyCode::Esc => {
                    app.cmd_state.exit();
                    return Ok(true);
                }
                KeyCode::Enter => {
                    app.cmd_state.command_mode = false;
                    app.cmd_state.command_history_idx = None;
                    app.cmd_state.history_draft = None;
                    app.execute_command(orchestrator);
                    app.cmd_state.command_buffer.clear();
                    app.cmd_state.cursor = 0;
                    return Ok(true);
                }
                KeyCode::Backspace => {
                    app.cmd_state.backspace();
                    return Ok(true);
                }
                KeyCode::Delete => {
                    app.cmd_state.delete();
                    return Ok(true);
                }
                KeyCode::Up => {
                    app.cmd_state.history_prev();
                    return Ok(true);
                }
                KeyCode::Down => {
                    app.cmd_state.history_next();
                    return Ok(true);
                }
                KeyCode::Tab => {
                    app.complete_command();
                    return Ok(true);
                }
                KeyCode::Left => {
                    app.cmd_state.move_left();
                    return Ok(true);
                }
                KeyCode::Right => {
                    app.cmd_state.move_right();
                    return Ok(true);
                }
                KeyCode::Home => {
                    app.cmd_state.move_home();
                    return Ok(true);
                }
                KeyCode::End => {
                    app.cmd_state.move_end();
                    return Ok(true);
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    app.cmd_state.insert_char(c);
                    return Ok(true);
                }
                _ => return Ok(false),
            }
        }

        if handle_help_search_panel(app, key) {
            return Ok(true);
        }

        // ── Wizard modal ──
        if app.wizard_state.active {
            match key.code {
                KeyCode::Enter => app.wizard_state.advance(),
                KeyCode::Esc => {
                    if app.wizard_state.step == super::wizard::WizardStep::Welcome {
                        app.wizard_state.active = false;
                    } else {
                        app.wizard_state.go_back();
                    }
                }
                _ => {}
            }
            return Ok(true);
        }

        // ── CH chain panel modal ──
        if app.ch_chain_panel_state.active {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    app.ch_chain_panel_state.active = false;
                }
                KeyCode::Tab => {
                    app.ch_chain_panel_state.cycle_focus();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    app.ch_chain_panel_state.select_prev();
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    app.ch_chain_panel_state.select_next();
                }
                KeyCode::Left => match app.ch_chain_panel_state.focus {
                    ChPanelFocus::ChainOrder => {
                        app.ch_chain_panel_state.move_up();
                        if !app.ch_chain_panel_state.clerics.is_empty() {
                            let members: Vec<u32> = app
                                .ch_chain_panel_state
                                .clerics
                                .iter()
                                .map(|cleric| cleric.pid)
                                .collect();
                            orchestrator.combat.ch_chain_set_members(members);
                            app.status_message = String::from("CH chain order updated");
                        }
                    }
                    ChPanelFocus::Timing => {
                        app.ch_chain_panel_state.chain_delay_secs =
                            (app.ch_chain_panel_state.chain_delay_secs - 0.25).max(0.0);
                        if let Some(chain) = &mut orchestrator.combat.ch_chain {
                            chain.set_interval(app.ch_chain_panel_state.chain_delay_secs);
                        } else {
                            app.status_message = String::from("No CH chain is running");
                        }
                    }
                    ChPanelFocus::Presets => {
                        app.ch_chain_panel_state.selected_preset =
                            app.ch_chain_panel_state.selected_preset.saturating_sub(1);
                    }
                    ChPanelFocus::Target => {
                        if let Some(first) = app.ch_chain_panel_state.clerics.first() {
                            app.status_message =
                                format!("Target focus: {} (PID {})", first.name, first.pid);
                        }
                    }
                },
                KeyCode::Right => match app.ch_chain_panel_state.focus {
                    ChPanelFocus::ChainOrder => {
                        app.ch_chain_panel_state.move_down();
                        if !app.ch_chain_panel_state.clerics.is_empty() {
                            let members: Vec<u32> = app
                                .ch_chain_panel_state
                                .clerics
                                .iter()
                                .map(|cleric| cleric.pid)
                                .collect();
                            orchestrator.combat.ch_chain_set_members(members);
                            app.status_message = String::from("CH chain order updated");
                        }
                    }
                    ChPanelFocus::Timing => {
                        app.ch_chain_panel_state.chain_delay_secs += 0.25;
                        if let Some(chain) = &mut orchestrator.combat.ch_chain {
                            chain.set_interval(app.ch_chain_panel_state.chain_delay_secs);
                        } else {
                            app.status_message = String::from("No CH chain is running");
                        }
                    }
                    ChPanelFocus::Presets => {
                        if !app.ch_chain_panel_state.presets.is_empty() {
                            let max = app.ch_chain_panel_state.presets.len() - 1;
                            if app.ch_chain_panel_state.selected_preset < max {
                                app.ch_chain_panel_state.selected_preset += 1;
                            }
                        }
                    }
                    ChPanelFocus::Target => {
                        if let Some(last) = app.ch_chain_panel_state.clerics.last() {
                            app.status_message =
                                format!("Target focus: {} (PID {})", last.name, last.pid);
                        }
                    }
                },
                KeyCode::Char('+') => {
                    app.ch_chain_panel_state.chain_delay_secs += 0.25;
                    if let Some(chain) = &mut orchestrator.combat.ch_chain {
                        chain.set_interval(app.ch_chain_panel_state.chain_delay_secs);
                    } else {
                        app.status_message = String::from("No CH chain is running");
                    }
                }
                KeyCode::Char('-') => {
                    app.ch_chain_panel_state.chain_delay_secs =
                        (app.ch_chain_panel_state.chain_delay_secs - 0.25).max(0.0);
                    if let Some(chain) = &mut orchestrator.combat.ch_chain {
                        chain.set_interval(app.ch_chain_panel_state.chain_delay_secs);
                    } else {
                        app.status_message = String::from("No CH chain is running");
                    }
                }
                KeyCode::Enter => {
                    if let Some(target) = app.target.as_ref() {
                        let target_id = target.spawn_id;
                        app.ch_chain_panel_state.target_id = target_id;
                        app.ch_chain_panel_state.target_name = target.displayed_name.clone();
                        if orchestrator.combat.ch_chain.is_some() {
                            orchestrator.combat.ch_chain_set_target(target_id);
                            app.status_message = format!(
                                "CH target set to {} ({})",
                                target.displayed_name, target_id
                            );
                        } else {
                            app.status_message = String::from("No CH chain is running");
                        }
                    } else {
                        app.status_message = String::from("No local target available");
                    }
                }
                KeyCode::Char('a') => {
                    if let Some(chain) = orchestrator.combat.ch_chain.as_mut() {
                        let next = !chain.is_adaptive();
                        chain.set_adaptive(next);
                        app.ch_chain_panel_state.adaptive = next;
                        app.status_message = if next {
                            String::from("CH adaptive mode: ON")
                        } else {
                            String::from("CH adaptive mode: OFF")
                        };
                    } else {
                        app.status_message = String::from("No CH chain is running");
                    }
                }
                KeyCode::Char('m') => {
                    if let Some(chain) = orchestrator.combat.ch_chain.as_mut() {
                        let mut members: Vec<u32> = app
                            .ch_chain_panel_state
                            .clerics
                            .iter()
                            .map(|cleric| cleric.pid)
                            .collect();
                        if let Some(member) = app
                            .ch_chain_panel_state
                            .clerics
                            .get(app.ch_chain_panel_state.selected)
                            .map(|m| m.pid)
                        {
                            members.retain(|pid| *pid != member);
                            chain.set_members(members);
                            app.sync_ch_chain_panel_state(orchestrator);
                            app.status_message = format!("Removed PID {member} from CH chain");
                        }
                    } else {
                        app.status_message = String::from("No CH chain is running");
                    }
                }
                _ => {}
            }

            app.sync_ch_chain_state(orchestrator);
            return Ok(true);
        }

        // ── Menu bar ──
        if key.modifiers.contains(KeyModifiers::ALT) {
            match key.code {
                KeyCode::F(10) | KeyCode::Char('m') | KeyCode::Char('M') => {
                    app.menu_state.toggle();
                    return Ok(true);
                }
                _ => {}
            }
        }

        if app.menu_state.active {
            match key.code {
                KeyCode::Esc | KeyCode::F(10) => {
                    app.menu_state.active = false;
                }
                KeyCode::Left => app.menu_state.prev_category(),
                KeyCode::Right => app.menu_state.next_category(),
                KeyCode::Up => app.menu_state.prev_item(),
                KeyCode::Down => app.menu_state.next_item(),
                KeyCode::Enter => {
                    let item = app.menu_state.selected_item().clone();
                    app.menu_state.active = false;
                    if item.requires_input {
                        app.cmd_state.enter(Some(&format!("{} ", item.command)));
                    } else {
                        app.cmd_state.command_buffer = item.command.to_string();
                        app.execute_command(orchestrator);
                        app.cmd_state.command_buffer.clear();
                        app.cmd_state.cursor = 0;
                    }
                }
                _ => {}
            }
            return Ok(true);
        }

        if app.help_visible {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => {
                    app.help_visible = false;
                    app.help_scroll = 0;
                    app.help_focus = None;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    app.help_scroll = app.help_scroll.saturating_add(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    app.help_scroll = app.help_scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    app.help_scroll = app.help_scroll.saturating_add(10);
                }
                KeyCode::PageUp => {
                    app.help_scroll = app.help_scroll.saturating_sub(10);
                }
                KeyCode::Home => {
                    app.help_scroll = 0;
                }
                _ => {}
            }
            return Ok(true);
        }

        if app.diagnostics_visible {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    app.toggle_diagnostics_panel();
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.toggle_diagnostics_panel();
                }
                KeyCode::Up | KeyCode::Char('k') => app.diagnostics_scroll_up(),
                KeyCode::Down | KeyCode::Char('j') => app.diagnostics_scroll_down(),
                KeyCode::PageUp => {
                    for _ in 0..5 {
                        app.diagnostics_scroll_up();
                    }
                }
                KeyCode::PageDown => {
                    for _ in 0..5 {
                        app.diagnostics_scroll_down();
                    }
                }
                KeyCode::Home => app.diagnostics_scroll = 0,
                _ => {}
            }
            return Ok(true);
        }

        if app.alert_panel_visible {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(8) => {
                    app.alert_panel_visible = false;
                }
                KeyCode::Up | KeyCode::Char('k') => app.select_prev_alert(),
                KeyCode::Down | KeyCode::Char('j') => app.select_next_alert(),
                KeyCode::Char('r') => app.refresh_alert_history(),
                KeyCode::Enter | KeyCode::Char('a') => {
                    if let Err(err) = app.acknowledge_selected_alert("tui") {
                        app.status_message = format!("Failed to acknowledge alert: {err}");
                    }
                }
                KeyCode::Char('A') => {
                    if let Err(err) = app.acknowledge_all_alerts("tui") {
                        app.status_message = format!("Failed to acknowledge all alerts: {err}");
                    }
                }
                _ => {}
            }
            return Ok(true);
        }

        if app.spawns_state.search_mode {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
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

        if handle_tactical_map_global_shortcut(app, key) {
            return Ok(true);
        }

        if handle_tactical_map_focused_shortcut(app, key) {
            return Ok(true);
        }

        if handle_orchestrator_dashboard_shortcut(app, orchestrator, key) {
            return Ok(true);
        }

        if handle_metrics_dashboard_shortcut(app, key) {
            return Ok(true);
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _) => {
                app.running = false;
                return Ok(true);
            }
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                app.cycle_layout();
                app.status_message = format!("Layout: {}", app.current_layout().label());
                return Ok(true);
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                app.toggle_diagnostics_panel();
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
            (KeyCode::Char('5'), _) => {
                app.set_active_screen(ActiveScreen::PacketMonitor);
                return Ok(true);
            }
            (KeyCode::Char('6'), _) => {
                app.set_active_screen(ActiveScreen::Economy);
                return Ok(true);
            }
            (KeyCode::Char('7'), _) => {
                app.set_active_screen(ActiveScreen::Orchestrator);
                return Ok(true);
            }
            (KeyCode::Char('8'), _) => {
                app.set_active_screen(ActiveScreen::Metrics);
                return Ok(true);
            }
            (KeyCode::Tab, _) => {
                app.toggle_panel();
                return Ok(true);
            }
            (KeyCode::BackTab, _) if app.keyboard_config.tab_navigation => {
                app.toggle_panel_reverse();
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
                if app.help_visible {
                    app.help_focus = Some(HelpFocus::Command(match app.active_screen {
                        ActiveScreen::Overview => "overview",
                        ActiveScreen::Tactical => "tactical",
                        ActiveScreen::Navigation => "navigation",
                        ActiveScreen::Debug => "debug",
                        ActiveScreen::PacketMonitor => "packet_monitor",
                        ActiveScreen::Economy => "economy",
                        ActiveScreen::Orchestrator => "orchestrator",
                        ActiveScreen::Metrics => "metrics",
                    }));
                    app.help_scroll = 0;
                } else {
                    app.help_focus = None;
                    app.help_scroll = 0;
                }
                return Ok(true);
            }
            (KeyCode::Char(':'), _) => {
                app.cmd_state.enter(None);
                return Ok(true);
            }
            (KeyCode::F(8), _) => {
                app.toggle_alert_panel();
                return Ok(true);
            }
            (KeyCode::Char('/'), _) => {
                open_help_search_panel(app);
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
            (KeyCode::Char('m'), _) if app.active_screen == ActiveScreen::Overview => {
                app.toggle_map();
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
            (KeyCode::F(n), KeyModifiers::CONTROL) if (1..=9).contains(&n) => {
                let hotkey = format!("F{n}");
                app.launch_profile_hotkey(&hotkey);
                return Ok(true);
            }
            (KeyCode::F(n), _) if (5..=9).contains(&n) => {
                let hotkey = format!("F{n}");
                if let Some(accounts) = &app.accounts_config {
                    if let Some(cp) = accounts.camera_preset_by_hotkey(&hotkey) {
                        let cmd = textquest_common::ipc::Command::SetCamera {
                            distance: cp.distance,
                            pitch: cp.pitch,
                            yaw: cp.yaw,
                        };
                        let ok = app.send_ipc_to_focused(&cmd);
                        if ok != 0 {
                            app.set_feedback(
                                ToastLevel::Success,
                                format!("Camera: {}", cp.name),
                                true,
                            );
                        }
                        return Ok(true);
                    }
                }
            }
            (KeyCode::F(10), _) => {
                app.menu_state.toggle();
                return Ok(true);
            }
            (KeyCode::F(4), _) => {
                app.open_web_onboarding();
                return Ok(true);
            }
            (KeyCode::F(n), _) if (1..=9).contains(&n) => {
                let idx = (n - 1) as usize;
                if let Some(cmd) = app
                    .cmd_state
                    .get_favorite(idx)
                    .map(std::string::ToString::to_string)
                {
                    app.cmd_state.command_buffer = cmd;
                    app.execute_command(orchestrator);
                    app.cmd_state.command_buffer.clear();
                } else {
                    app.status_message =
                        format!("F{n}: no favorite assigned (use commands to build frequency)");
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
                KeyCode::Char('<') => {
                    app.map_state.decrease_z_filter();
                    return Ok(true);
                }
                KeyCode::Char('>') => {
                    app.map_state.increase_z_filter();
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
            ActivePanel::TacticalMap => match key.code {
                KeyCode::Enter => {
                    app.toggle_tactical_map_maximized();
                    return Ok(true);
                }
                KeyCode::Left => {
                    app.pan_tactical_map_left();
                    return Ok(true);
                }
                KeyCode::Right => {
                    app.pan_tactical_map_right();
                    return Ok(true);
                }
                KeyCode::Up => {
                    app.pan_tactical_map_up();
                    return Ok(true);
                }
                KeyCode::Down => {
                    app.pan_tactical_map_down();
                    return Ok(true);
                }
                KeyCode::PageUp | KeyCode::Char('+' | '=') => {
                    app.zoom_tactical_map_in();
                    return Ok(true);
                }
                KeyCode::PageDown | KeyCode::Char('-' | '_') => {
                    app.zoom_tactical_map_out();
                    return Ok(true);
                }
                key if handle_tactical_map_panel_toggle(app, key) => {
                    return Ok(true);
                }
                _ => {}
            },
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
                KeyCode::Char('p' | 'P') if app.active_screen == ActiveScreen::Debug => {
                    app.toggle_gemma_observer_pause();
                    return Ok(true);
                }
                KeyCode::Down | KeyCode::Char('j') => app.spawn_list_down(),
                KeyCode::Up | KeyCode::Char('k') => app.spawn_list_up(),
                KeyCode::PageDown => app.spawn_list_page_down(),
                KeyCode::PageUp => app.spawn_list_page_up(),
                KeyCode::Home => app.spawns_state.table_state.select(Some(0)),
                KeyCode::End => {
                    let max = app.filtered_spawn_count().saturating_sub(1);
                    app.spawns_state.table_state.select(Some(max));
                }
                KeyCode::Enter => app.navigate_to_selected_spawn(),
                KeyCode::Char('h') | KeyCode::Char('x') => app.debug_selected_spawn(),
                KeyCode::Char('a') => app.target_selected_spawn(),
                KeyCode::Char('s') => app.cycle_spawn_sort(),
                KeyCode::Char('S') => app.toggle_spawn_sort_direction(),
                KeyCode::Char('n') => app.cycle_nav_scope(),
                _ => {}
            },
            ActivePanel::DebugHexDump => match key.code {
                KeyCode::Char('p' | 'P') => {
                    app.toggle_gemma_observer_pause();
                    return Ok(true);
                }
                KeyCode::Down => app.hex_scroll_down(),
                KeyCode::Up => app.hex_scroll_up(),
                KeyCode::Char('a') => app.toggle_hex_annotations(),
                _ => {}
            },
            ActivePanel::DebugExplorer => {
                if !app.explorer_state.search_mode && matches!(key.code, KeyCode::Char('p' | 'P'))
                {
                    app.toggle_gemma_observer_pause();
                    return Ok(true);
                }
                if handle_debug_explorer_panel_key(app, key.code) {
                    return Ok(true);
                }
            }
            ActivePanel::PacketMonitorLog => {
                if handle_packet_monitor_log_key(app, key.code) {
                    return Ok(true);
                }
            }
            ActivePanel::DebugInternals => {
                if app.eq_internals_state.search_mode {
                    match key.code {
                        KeyCode::Esc => {
                            app.eq_internals_state.search_mode = false;
                        }
                        KeyCode::Enter => {
                            app.eq_internals_state.search_mode = false;
                            app.eq_internals_state.apply_filter();
                        }
                        KeyCode::Backspace => {
                            app.eq_internals_state.search_filter.pop();
                            app.eq_internals_state.apply_filter();
                        }
                        KeyCode::Char(ch) => {
                            app.eq_internals_state.search_filter.push(ch);
                            app.eq_internals_state.apply_filter();
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.eq_internals_state.select_next();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.eq_internals_state.select_prev();
                        }
                        KeyCode::Enter => {
                            app.internals_select_offset();
                        }
                        KeyCode::Char('c') => {
                            app.eq_internals_state.category_filter =
                                app.eq_internals_state.category_filter.next();
                            app.eq_internals_state.apply_filter();
                        }
                        KeyCode::Char('/') => {
                            app.eq_internals_state.search_mode = true;
                        }
                        KeyCode::Char('p' | 'P') => {
                            app.toggle_gemma_observer_pause();
                            return Ok(true);
                        }
                        _ => {}
                    }
                }
            }
            ActivePanel::EconomyControls => match key.code {
                KeyCode::Char('p' | 'P') => {
                    app.economy_state.vendor_status =
                        crate::tui::ui::economy_controls::VendorCycleStatus::Paused;
                    app.economy_state.automation_paused = true;
                    app.status_message = String::from("Economy: cycle PAUSED");
                    return Ok(true);
                }
                KeyCode::Char('r' | 'R') => {
                    app.economy_state.vendor_status =
                        crate::tui::ui::economy_controls::VendorCycleStatus::Idle;
                    app.economy_state.automation_paused = false;
                    app.status_message = String::from("Economy: cycle RESUMED");
                    return Ok(true);
                }
                KeyCode::Char('a' | 'A') => {
                    app.economy_state.vendor_status =
                        crate::tui::ui::economy_controls::VendorCycleStatus::Aborted;
                    app.economy_state.automation_paused = true;
                    app.status_message = String::from("Economy: cycle ABORTED");
                    return Ok(true);
                }
                KeyCode::Char('s' | 'S') => {
                    app.economy_state.vendor_next_cycle_secs = 0;
                    app.status_message =
                        String::from("Economy: cycle SKIPPED — will trigger next tick");
                    return Ok(true);
                }
                _ => {}
            },
            _ => match key.code {
                KeyCode::Home if !app.automation_paused => {
                    app.automation_paused = true;
                    app.status_message = String::from("Automation PAUSED");
                    tracing::info!("operator paused automation via HOME key");
                }
                KeyCode::End if app.automation_paused => {
                    app.automation_paused = false;
                    app.status_message = String::from("Automation RESUMED");
                    tracing::info!("operator resumed automation via END key");
                }
                _ => {}
            },
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::MapViewportMode;
    use crate::{
        eq::structs::{SpawnInfo, SpawnType, StandState},
        tui::state::MapFilterKind,
    };

    fn test_local_player(name: &str) -> SpawnInfo {
        SpawnInfo {
            name: name.into(),
            displayed_name: name.into(),
            lastname: String::new(),
            spawn_id: 1,
            spawn_type: SpawnType::Player,
            level: 60,
            class_id: 1,
            class: None,
            stand_state: StandState::Standing,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 100,
            hp_max: 100,
            mana_current: 100,
            mana_max: 100,
            endurance_current: 100,
            endurance_max: 100,
            is_gm: false,
            race_id: 1,
            buff_slots: Vec::new(),
            spellbook: Vec::new(),
            memorized_spells: Vec::new(),
            cast_state: None,
        }
    }

    #[test]
    fn tactical_map_global_alt_filter_shortcut_toggles_npc_filter() {
        let mut app = App::new();
        app.active_screen = ActiveScreen::Tactical;
        app.active_panel = ActivePanel::TacticalSpawns;

        assert!(handle_tactical_map_global_shortcut(
            &mut app,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::ALT),
        ));
        assert_eq!(app.active_panel, ActivePanel::TacticalMap);
        assert_eq!(app.status_message, "NPC filter OFF");
        assert!(!app.map_state.filters.show_npc);
    }

    #[test]
    fn tactical_map_global_alt_navmesh_shortcut_uses_overlay_loader() {
        let mut app = App::new();
        app.active_screen = ActiveScreen::Tactical;
        app.active_panel = ActivePanel::TacticalSpawns;
        app.map_state.show_navmesh = false;

        assert!(handle_tactical_map_global_shortcut(
            &mut app,
            KeyEvent::new(KeyCode::Char('4'), KeyModifiers::ALT),
        ));
        assert!(app.map_state.show_navmesh);
        assert_eq!(app.active_panel, ActivePanel::TacticalMap);
        assert!(app.status_message.contains("navmesh overlay"));
    }

    #[test]
    fn tactical_map_panel_toggle_supports_annotations_and_filters() {
        let mut app = App::new();

        assert!(handle_tactical_map_panel_toggle(
            &mut app,
            KeyCode::Char('a')
        ));
        assert!(app.map_state.show_annotations);
        assert_eq!(app.status_message, "Annotations ON");

        assert!(handle_tactical_map_panel_toggle(
            &mut app,
            KeyCode::Char('P')
        ));
        assert_eq!(app.status_message, "PC filter OFF");
        assert!(!app.map_state.filters.show_pc);
    }

    #[test]
    fn tactical_map_panel_toggle_e_toggles_extended_layer() {
        let mut app = App::new();
        assert!(!app.map_state.show_extended);

        assert!(handle_tactical_map_panel_toggle(
            &mut app,
            KeyCode::Char('e')
        ));
        assert!(app.map_state.show_extended);
        assert_eq!(app.status_message, "Extended ON");

        assert!(handle_tactical_map_panel_toggle(
            &mut app,
            KeyCode::Char('e')
        ));
        assert!(!app.map_state.show_extended);
        assert_eq!(app.status_message, "Extended OFF");
    }

    #[test]
    fn debug_explorer_enter_loads_selected_row_into_hex() {
        let mut app = App::new();
        app.active_screen = ActiveScreen::Debug;
        app.active_panel = ActivePanel::DebugExplorer;
        app.explorer_state.all_functions = vec![crate::tui::state::ExplorerEntry {
            address: 0x1400_1234,
            name: String::from("ProcessGameEvents"),
            category: Some(String::from("ghidra_auto")),
            usability: Some(String::from("untested")),
            size: Some(64),
        }];
        app.explorer_state.apply_filter();

        assert!(handle_debug_explorer_panel_key(&mut app, KeyCode::Enter));

        assert_eq!(app.active_panel, ActivePanel::DebugHexDump);
        assert!(app.hex_state.hex_label.contains("ProcessGameEvents"));
    }

    #[test]
    fn packet_monitor_space_toggles_pause_status() {
        let mut app = App::new();

        assert!(handle_packet_monitor_log_key(&mut app, KeyCode::Char(' ')));
        assert!(app.packet_monitor_state.paused);
        assert_eq!(app.status_message, "Packet monitor paused");

        assert!(handle_packet_monitor_log_key(&mut app, KeyCode::Char(' ')));
        assert!(!app.packet_monitor_state.paused);
        assert_eq!(app.status_message, "Packet monitor resumed");
    }

    #[test]
    fn tactical_map_global_alt7_toggles_extended_layer() {
        let mut app = App::new();
        app.active_screen = ActiveScreen::Tactical;
        assert!(!app.map_state.show_extended);

        assert!(handle_tactical_map_global_shortcut(
            &mut app,
            KeyEvent::new(KeyCode::Char('7'), KeyModifiers::ALT),
        ));
        assert!(app.map_state.show_extended);
        assert_eq!(app.status_message, "Extended ON");
    }

    #[test]
    fn tactical_map_focused_shortcuts_set_view_modes_and_fit_zone() {
        let mut app = App::new();
        app.active_screen = ActiveScreen::Tactical;
        app.active_panel = ActivePanel::TacticalMap;
        app.local_player = Some(test_local_player("Observer"));

        assert!(handle_tactical_map_focused_shortcut(
            &mut app,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
        ));
        assert_eq!(app.map_state.viewport_mode, MapViewportMode::Local);
        assert_eq!(app.status_message, "Map: local view");

        assert!(handle_tactical_map_focused_shortcut(
            &mut app,
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
        ));
        assert_eq!(app.map_state.viewport_mode, MapViewportMode::Global);
        assert_eq!(app.status_message, "Map: zone fit");

        assert!(handle_tactical_map_focused_shortcut(
            &mut app,
            KeyEvent::new(KeyCode::Char('I'), KeyModifiers::SHIFT),
        ));
        assert!(app.status_message.starts_with("Map info: "));
    }

    #[test]
    fn tactical_map_focused_shortcuts_consume_unavailable_local_view() {
        let mut app = App::new();
        app.active_screen = ActiveScreen::Tactical;
        app.active_panel = ActivePanel::TacticalMap;
        app.map_state.viewport_mode = MapViewportMode::Global;

        assert!(handle_tactical_map_focused_shortcut(
            &mut app,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
        ));
        assert_eq!(app.map_state.viewport_mode, MapViewportMode::Global);
        assert_eq!(app.status_message, "Map: local view unavailable");
    }

    #[test]
    fn tactical_map_focused_shortcuts_prefer_map_controls_over_global_loot() {
        let mut app = App::new();
        app.active_screen = ActiveScreen::Tactical;
        app.active_panel = ActivePanel::TacticalMap;

        assert!(handle_tactical_map_focused_shortcut(
            &mut app,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
        ));
        assert!(app.map_state.show_labels);
        assert_eq!(app.status_message, "Labels ON");

        assert!(handle_tactical_map_focused_shortcut(
            &mut app,
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
        ));
        assert!(!app.map_state.filters.show_ground);
        assert_eq!(
            app.status_message,
            MapFilterKind::Ground.toggle_status(false)
        );
    }

    #[test]
    fn orchestrator_dashboard_shortcut_cycles_tabs() {
        let mut app = App::new();
        let mut orchestrator = Orchestrator::new();
        app.set_active_screen(ActiveScreen::Orchestrator);

        assert!(handle_orchestrator_dashboard_shortcut(
            &mut app,
            &mut orchestrator,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        ));
        assert_eq!(app.orchestrator_state.active_tab.label(), "Group");
    }

    #[test]
    fn orchestrator_dashboard_shortcut_toggles_pause() {
        let mut app = App::new();
        let mut orchestrator = Orchestrator::new();
        app.set_active_screen(ActiveScreen::Orchestrator);

        assert!(handle_orchestrator_dashboard_shortcut(
            &mut app,
            &mut orchestrator,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        ));
        assert!(app.automation_paused);
        assert_eq!(app.status_message, "Automation PAUSED");
    }

    #[test]
    fn orchestrator_dashboard_shortcut_does_not_override_tab_key() {
        let mut app = App::new();
        let mut orchestrator = Orchestrator::new();
        app.set_active_screen(ActiveScreen::Orchestrator);

        assert!(!handle_orchestrator_dashboard_shortcut(
            &mut app,
            &mut orchestrator,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        ));
        assert_eq!(app.orchestrator_state.active_tab.label(), "Session");
    }
}
