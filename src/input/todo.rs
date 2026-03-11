use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::App;

pub(super) fn handle_todo_panel_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    let is_editing = app
        .todo_panel
        .as_ref()
        .map(|s| s.editing.is_some())
        .unwrap_or(false);

    if is_editing {
        return handle_todo_edit_keys(app, key);
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.close_todo_panel();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut state) = app.todo_panel {
                state.move_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut state) = app.todo_panel {
                state.move_down();
            }
        }
        KeyCode::Char(' ') => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(todo) = state.todos.get_mut(state.selected) {
                    todo.status = match todo.status.as_str() {
                        "pending" => "in_progress",
                        "in_progress" => "completed",
                        _ => "pending",
                    }
                    .to_string();
                    state.dirty = true;
                    let session_id = state.session_id.clone();
                    let todos = state.todos.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                            tracing::error!("Failed to save todos: {e}");
                        }
                    });
                }
            }
        }
        KeyCode::Char('n') => {
            if let Some(ref mut state) = app.todo_panel {
                state.editing = Some(crate::app::EditingState {
                    index: None,
                    buffer: String::new(),
                    cursor_pos: 0,
                    priority: "high".to_string(),
                });
                state.selected = state.todos.len();
            }
        }
        KeyCode::Char('e') => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(todo) = state.todos.get(state.selected).cloned() {
                    state.editing = Some(crate::app::EditingState {
                        index: Some(state.selected),
                        buffer: todo.content.clone(),
                        cursor_pos: todo.content.len(),
                        priority: todo.priority.clone(),
                    });
                }
            }
        }
        KeyCode::Char('d') => {
            if let Some(ref mut state) = app.todo_panel {
                if !state.todos.is_empty() {
                    state.todos.remove(state.selected);
                    if state.selected >= state.todos.len() {
                        state.selected = state.todos.len().saturating_sub(1);
                    }
                    state.dirty = true;
                    let session_id = state.session_id.clone();
                    let todos = state.todos.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                            tracing::error!("Failed to save todos: {e}");
                        }
                    });
                }
            }
        }
        KeyCode::Char('p') => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(todo) = state.todos.get_mut(state.selected) {
                    todo.priority = match todo.priority.as_str() {
                        "low" => "medium",
                        "medium" => "high",
                        _ => "low",
                    }
                    .to_string();
                    state.dirty = true;
                    let session_id = state.session_id.clone();
                    let todos = state.todos.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                            tracing::error!("Failed to save todos: {e}");
                        }
                    });
                }
            }
        }
        KeyCode::Char('K') => {
            if let Some(ref mut state) = app.todo_panel {
                if state.selected > 0 {
                    state.todos.swap(state.selected, state.selected - 1);
                    state.selected -= 1;
                    state.dirty = true;
                    let session_id = state.session_id.clone();
                    let todos = state.todos.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                            tracing::error!("Failed to save todos: {e}");
                        }
                    });
                }
            }
        }
        KeyCode::Char('J') => {
            if let Some(ref mut state) = app.todo_panel {
                if state.selected + 1 < state.todos.len() {
                    state.todos.swap(state.selected, state.selected + 1);
                    state.selected += 1;
                    state.dirty = true;
                    let session_id = state.session_id.clone();
                    let todos = state.todos.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                            tracing::error!("Failed to save todos: {e}");
                        }
                    });
                }
            }
        }
        KeyCode::Char('y') => {
            if let Some(ref state) = app.todo_panel {
                if let Some(todo) = state.todos.get(state.selected) {
                    let _ = std::process::Command::new("pbcopy")
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                        .and_then(|mut child| {
                            use std::io::Write;
                            if let Some(ref mut stdin) = child.stdin {
                                stdin.write_all(todo.content.as_bytes())?;
                            }
                            child.wait()
                        });
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn handle_todo_edit_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    let is_editing = app
        .todo_panel
        .as_ref()
        .map(|s| s.editing.is_some())
        .unwrap_or(false);
    if !is_editing {
        return Ok(());
    }

    match key.code {
        KeyCode::Esc => {
            if let Some(ref mut state) = app.todo_panel {
                state.editing = None;
            }
        }
        KeyCode::Enter => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(ref editing) = state.editing {
                    let content = editing.buffer.trim().to_string();
                    let priority = editing.priority.clone();
                    let index = editing.index;
                    if !content.is_empty() {
                        if let Some(idx) = index {
                            if let Some(todo) = state.todos.get_mut(idx) {
                                todo.content = content;
                                todo.priority = priority;
                            }
                        } else {
                            state.todos.push(crate::app::TodoItem {
                                content,
                                status: "pending".to_string(),
                                priority,
                            });
                        }
                        state.dirty = true;
                        let session_id = state.session_id.clone();
                        let todos = state.todos.clone();
                        tokio::task::spawn_blocking(move || {
                            if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                                tracing::error!("Failed to save todos: {e}");
                            }
                        });
                    }
                }
                state.editing = None;
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(ref mut editing) = state.editing {
                    editing.buffer.insert(editing.cursor_pos, c);
                    editing.cursor_pos += c.len_utf8();
                }
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(ref mut editing) = state.editing {
                    if editing.cursor_pos > 0 {
                        let prev = editing.buffer[..editing.cursor_pos]
                            .char_indices()
                            .next_back()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        editing.buffer.replace_range(prev..editing.cursor_pos, "");
                        editing.cursor_pos = prev;
                    }
                }
            }
        }
        KeyCode::Left => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(ref mut editing) = state.editing {
                    if editing.cursor_pos > 0 {
                        editing.cursor_pos = editing.buffer[..editing.cursor_pos]
                            .char_indices()
                            .next_back()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                    }
                }
            }
        }
        KeyCode::Right => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(ref mut editing) = state.editing {
                    if editing.cursor_pos < editing.buffer.len() {
                        editing.cursor_pos = editing.buffer[editing.cursor_pos..]
                            .char_indices()
                            .nth(1)
                            .map(|(i, _)| editing.cursor_pos + i)
                            .unwrap_or(editing.buffer.len());
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}
