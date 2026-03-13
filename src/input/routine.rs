use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, BackgroundEvent, RoutineEditState, RoutineItem};

/// Spawn an async task to fetch routines from the web state handle.
/// Sends `RoutinesFetched` on success or silently logs on failure.
pub(crate) fn spawn_fetch_routines(app: &App) {
    let Some(ref wsh) = app.web_state else { return };
    let wsh = wsh.clone();
    let bg_tx = app.bg_tx.clone();
    tokio::spawn(async move {
        let (defs, _runs) = wsh.list_routines().await;
        let routines: Vec<RoutineItem> = defs.iter().map(RoutineItem::from_definition).collect();
        let _ = bg_tx.send(BackgroundEvent::RoutinesFetched { routines });
    });
}

pub(super) fn handle_routine_panel_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    // --- Delete confirmation mode ---
    if let Some(ref mut state) = app.routine_panel {
        if state.confirm_delete.is_some() {
            return handle_confirm_delete_keys(app, key);
        }
    }

    // --- Editing mode (create/edit form) ---
    if let Some(ref mut state) = app.routine_panel {
        if state.editing.is_some() {
            return handle_editing_keys(app, key);
        }
    }

    // --- Normal list mode ---
    handle_list_keys(app, key)
}

/// Handle keys in normal list mode (existing behavior + new create/edit/delete triggers).
fn handle_list_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.routine_panel = None;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut state) = app.routine_panel {
                state.move_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut state) = app.routine_panel {
                state.move_down();
            }
        }
        // Enter or Space: run the selected routine
        KeyCode::Enter | KeyCode::Char(' ') => {
            if let Some(ref mut state) = app.routine_panel {
                if state.running.is_some() {
                    return Ok(());
                }
                if let Some(routine) = state.routines.get(state.selected) {
                    let routine_id = routine.id.clone();
                    let routine_name = routine.name.clone();
                    state.running = Some(routine_id.clone());

                    if let Some(ref wsh) = app.web_state {
                        let wsh = wsh.clone();
                        let bg_tx = app.bg_tx.clone();
                        tokio::spawn(async move {
                            let (success, message) = match wsh.execute_routine(&routine_id).await {
                                Ok(_run) => (true, routine_name),
                                Err(e) => (false, format!("Failed: {e}")),
                            };
                            let _ = bg_tx.send(BackgroundEvent::RoutineRunCompleted {
                                routine_id: routine_id.clone(),
                                success,
                                message,
                            });
                            // Refetch routines to update last_run_at etc.
                            let (defs, _) = wsh.list_routines().await;
                            let routines: Vec<RoutineItem> =
                                defs.iter().map(RoutineItem::from_definition).collect();
                            let _ = bg_tx.send(BackgroundEvent::RoutinesFetched { routines });
                        });
                    }
                }
            }
        }
        // 'e' — toggle enabled/disabled
        KeyCode::Char('e') => {
            if let Some(ref mut state) = app.routine_panel {
                if let Some(routine) = state.routines.get(state.selected).cloned() {
                    let new_enabled = !routine.enabled;
                    let routine_id = routine.id.clone();
                    // Optimistically update
                    if let Some(r) = state.routines.get_mut(state.selected) {
                        r.enabled = new_enabled;
                    }
                    if let Some(ref wsh) = app.web_state {
                        let wsh = wsh.clone();
                        let bg_tx = app.bg_tx.clone();
                        tokio::spawn(async move {
                            let req = crate::web::types::UpdateRoutineRequest {
                                name: None,
                                trigger: None,
                                action: None,
                                enabled: Some(new_enabled),
                                cron_expr: None,
                                timezone: None,
                                target_mode: None,
                                session_id: None,
                                project_index: None,
                                prompt: None,
                                provider_id: None,
                                model_id: None,
                                mission_id: None,
                            };
                            let _ = wsh.update_routine(&routine_id, req).await;
                            // Refetch
                            let (defs, _) = wsh.list_routines().await;
                            let routines: Vec<RoutineItem> =
                                defs.iter().map(RoutineItem::from_definition).collect();
                            let _ = bg_tx.send(BackgroundEvent::RoutinesFetched { routines });
                        });
                    }
                }
            }
        }
        // 'r' — refresh list
        KeyCode::Char('r') => {
            spawn_fetch_routines(app);
        }
        // 'd' — toggle detail pane for selected routine
        KeyCode::Char('d') => {
            if let Some(ref mut state) = app.routine_panel {
                state.show_detail = !state.show_detail;
            }
        }
        // 'n' or 'c' — create new routine
        KeyCode::Char('n') | KeyCode::Char('c') => {
            if let Some(ref mut state) = app.routine_panel {
                state.editing = Some(RoutineEditState::new_create());
                state.show_detail = false;
            }
        }
        // 'E' (capital) — edit selected routine
        KeyCode::Char('E') => {
            if let Some(ref mut state) = app.routine_panel {
                if let Some(routine) = state.routines.get(state.selected).cloned() {
                    state.editing = Some(RoutineEditState::from_routine(&routine));
                    state.show_detail = false;
                }
            }
        }
        // 'x' or Delete — delete selected routine (start confirmation)
        KeyCode::Char('x') | KeyCode::Delete => {
            if let Some(ref mut state) = app.routine_panel {
                if let Some(routine) = state.routines.get(state.selected) {
                    state.confirm_delete = Some(routine.id.clone());
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Handle keys when a delete confirmation is pending.
fn handle_confirm_delete_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    let routine_id = {
        let state = app.routine_panel.as_ref().unwrap();
        match &state.confirm_delete {
            Some(id) => id.clone(),
            None => return Ok(()),
        }
    };

    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            // Confirm: spawn delete task
            if let Some(ref wsh) = app.web_state {
                let wsh = wsh.clone();
                let bg_tx = app.bg_tx.clone();
                let rid = routine_id.clone();
                tokio::spawn(async move {
                    let success = wsh.delete_routine(&rid).await;
                    let _ = bg_tx.send(BackgroundEvent::RoutineDeleted {
                        routine_id: rid.clone(),
                        success,
                    });
                    // Refetch to stay in sync
                    let (defs, _) = wsh.list_routines().await;
                    let routines: Vec<RoutineItem> =
                        defs.iter().map(RoutineItem::from_definition).collect();
                    let _ = bg_tx.send(BackgroundEvent::RoutinesFetched { routines });
                });
            }
            // Clear confirmation (will also be cleared on RoutineDeleted event)
            if let Some(ref mut state) = app.routine_panel {
                state.confirm_delete = None;
            }
        }
        // Any other key cancels
        _ => {
            if let Some(ref mut state) = app.routine_panel {
                state.confirm_delete = None;
            }
        }
    }
    Ok(())
}

/// Handle keys when the create/edit form is active.
fn handle_editing_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    // Ctrl+S: save
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
        return save_editing(app);
    }

    // Check if Enter on last field (needs save) — check before mutable borrow
    if key.code == KeyCode::Enter {
        let should_save = {
            let state = app.routine_panel.as_ref().unwrap();
            let edit = state.editing.as_ref().unwrap();
            edit.focused_field >= edit.field_count - 1
        };
        if should_save {
            return save_editing(app);
        }
    }

    let state = app.routine_panel.as_mut().unwrap();
    let edit = state.editing.as_mut().unwrap();

    match key.code {
        KeyCode::Esc => {
            // Cancel editing
            state.editing = None;
        }
        KeyCode::Tab | KeyCode::Down => {
            if !key.modifiers.contains(KeyModifiers::SHIFT) {
                edit.focus_next();
            } else {
                edit.focus_prev();
            }
        }
        KeyCode::BackTab | KeyCode::Up => {
            edit.focus_prev();
        }
        KeyCode::Enter => {
            // Not on last field (checked above), just advance
            edit.focus_next();
        }
        KeyCode::Char(ch) => {
            match edit.focused_field {
                // 0 = name (text)
                0 => edit.name.push(ch),
                // 1 = trigger (toggle: cycle on space)
                1 => {
                    if ch == ' ' {
                        cycle_trigger(edit);
                    }
                }
                // 2 = prompt (text)
                2 => edit.prompt.push(ch),
                // 3 = target_mode (toggle: cycle on space)
                3 => {
                    if ch == ' ' {
                        cycle_target_mode(edit);
                    }
                }
                // 4 = cron_expr (text)
                4 => edit.cron_expr.push(ch),
                // 5 = enabled (toggle: cycle on space)
                5 => {
                    if ch == ' ' {
                        edit.enabled = !edit.enabled;
                    }
                }
                _ => {}
            }
        }
        KeyCode::Backspace => {
            match edit.focused_field {
                0 => {
                    edit.name.pop();
                }
                2 => {
                    edit.prompt.pop();
                }
                4 => {
                    edit.cron_expr.pop();
                }
                _ => {}
            }
        }
        KeyCode::Left => {
            // For toggle fields, cycle backward
            match edit.focused_field {
                1 => cycle_trigger(edit),
                3 => cycle_target_mode(edit),
                5 => edit.enabled = !edit.enabled,
                _ => {}
            }
        }
        KeyCode::Right => {
            // For toggle fields, cycle forward
            match edit.focused_field {
                1 => cycle_trigger(edit),
                3 => cycle_target_mode(edit),
                5 => edit.enabled = !edit.enabled,
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

fn cycle_trigger(edit: &mut RoutineEditState) {
    edit.trigger = match edit.trigger.as_str() {
        "manual" => "scheduled".to_string(),
        "scheduled" => "on_session_idle".to_string(),
        "on_session_idle" => "daily_summary".to_string(),
        "daily_summary" => "manual".to_string(),
        _ => "manual".to_string(),
    };
}

fn cycle_target_mode(edit: &mut RoutineEditState) {
    edit.target_mode = match edit.target_mode.as_str() {
        "new_session" => "existing_session".to_string(),
        "existing_session" => "new_session".to_string(),
        _ => "new_session".to_string(),
    };
}

/// Save the current editing state by calling create or update.
fn save_editing(app: &mut App) -> Result<()> {
    let state = app.routine_panel.as_mut().unwrap();
    let edit = state.editing.as_ref().unwrap();

    // Validate name is non-empty
    if edit.name.trim().is_empty() {
        return Ok(());
    }

    let trigger = match edit.trigger.as_str() {
        "manual" => crate::web::types::RoutineTrigger::Manual,
        "scheduled" => crate::web::types::RoutineTrigger::Scheduled,
        "on_session_idle" => crate::web::types::RoutineTrigger::OnSessionIdle,
        "daily_summary" => crate::web::types::RoutineTrigger::DailySummary,
        _ => crate::web::types::RoutineTrigger::Manual,
    };

    let target_mode = match edit.target_mode.as_str() {
        "existing_session" => Some(crate::web::types::RoutineTargetMode::ExistingSession),
        "new_session" => Some(crate::web::types::RoutineTargetMode::NewSession),
        _ => Some(crate::web::types::RoutineTargetMode::NewSession),
    };

    let cron_expr = if edit.cron_expr.trim().is_empty() {
        None
    } else {
        Some(edit.cron_expr.trim().to_string())
    };

    let prompt = if edit.prompt.trim().is_empty() {
        None
    } else {
        Some(edit.prompt.clone())
    };

    let is_create = edit.routine_id.is_none();
    let routine_id = edit.routine_id.clone();
    let name = edit.name.clone();
    let enabled = edit.enabled;

    // Clear editing state
    state.editing = None;

    if let Some(ref wsh) = app.web_state {
        let wsh = wsh.clone();
        let bg_tx = app.bg_tx.clone();

        if is_create {
            let req = crate::web::types::CreateRoutineRequest {
                name,
                trigger,
                action: crate::web::types::RoutineAction::SendMessage,
                enabled,
                cron_expr,
                timezone: None,
                target_mode,
                session_id: None,
                project_index: None,
                prompt,
                provider_id: None,
                model_id: None,
                mission_id: None,
            };
            tokio::spawn(async move {
                let def = wsh.create_routine(req).await;
                let item = RoutineItem::from_definition(&def);
                let _ = bg_tx.send(BackgroundEvent::RoutineCreated { routine: item });
                // Refetch to stay in sync
                let (defs, _) = wsh.list_routines().await;
                let routines: Vec<RoutineItem> =
                    defs.iter().map(RoutineItem::from_definition).collect();
                let _ = bg_tx.send(BackgroundEvent::RoutinesFetched { routines });
            });
        } else {
            let rid = routine_id.unwrap();
            let req = crate::web::types::UpdateRoutineRequest {
                name: Some(name),
                trigger: Some(trigger),
                action: Some(crate::web::types::RoutineAction::SendMessage),
                enabled: Some(enabled),
                cron_expr: Some(cron_expr),
                timezone: None,
                target_mode: Some(target_mode),
                session_id: None,
                project_index: None,
                prompt: Some(prompt),
                provider_id: None,
                model_id: None,
                mission_id: None,
            };
            tokio::spawn(async move {
                let _ = wsh.update_routine(&rid, req).await;
                // Refetch to stay in sync
                let (defs, _) = wsh.list_routines().await;
                let routines: Vec<RoutineItem> =
                    defs.iter().map(RoutineItem::from_definition).collect();
                let _ = bg_tx.send(BackgroundEvent::RoutinesFetched { routines });
            });
        }
    }
    Ok(())
}
