use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::ui::layout_manager::PanelId;
use crate::vim_mode::VimMode;

pub(super) fn handle_session_search_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.close_session_search();
        }
        KeyCode::Enter => {
            if let Some(session_id) = app.pin_selected_session() {
                app.pending_session_select = Some((app.active_project, session_id));
                app.layout.focused = PanelId::TerminalPane;
            }
        }
        KeyCode::Up => {
            if app.session_search_selected > 0 {
                app.session_search_selected -= 1;
            }
        }
        KeyCode::Down => {
            let max = app.session_search_results.len().saturating_sub(1);
            if app.session_search_selected < max {
                app.session_search_selected += 1;
            }
        }
        KeyCode::Backspace => {
            if app.session_search_cursor > 0 {
                app.session_search_cursor -= 1;
                app.session_search_buffer.remove(app.session_search_cursor);
                app.update_session_search();
            }
        }
        KeyCode::Char(c) => {
            app.session_search_buffer
                .insert(app.session_search_cursor, c);
            app.session_search_cursor += 1;
            app.update_session_search();
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn handle_session_selector_keys(app: &mut App, key: &KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.session_selector = None;
            app.vim_mode = VimMode::Normal;
        }
        KeyCode::Enter => {
            let switch_info = app.session_selector.as_ref().and_then(|state| {
                if state.filtered.is_empty() {
                    return None;
                }
                let entry = &state.entries[state.filtered[state.selected]];
                Some((entry.project_idx, entry.session.id.clone()))
            });
            if let Some((project_idx, session_id)) = switch_info {
                app.active_project = project_idx;
                app.projects[project_idx].active_session = Some(session_id.clone());
                app.pending_session_select = Some((project_idx, session_id));
                app.session_selector = None;
                app.vim_mode = VimMode::Insert;
                app.layout.focused = PanelId::TerminalPane;
            }
        }
        KeyCode::Up | KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut state) = app.session_selector {
                state.move_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut state) = app.session_selector {
                state.move_down();
            }
        }
        KeyCode::Up => {
            if let Some(ref mut state) = app.session_selector {
                state.move_up();
            }
        }
        KeyCode::Down => {
            if let Some(ref mut state) = app.session_selector {
                state.move_down();
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut state) = app.session_selector {
                state.backspace();
            }
        }
        KeyCode::Left => {
            if let Some(ref mut state) = app.session_selector {
                state.cursor_left();
            }
        }
        KeyCode::Right => {
            if let Some(ref mut state) = app.session_selector {
                state.cursor_right();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut state) = app.session_selector {
                state.insert_char(c);
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn handle_add_project_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            if app.completions_visible {
                app.completions_visible = false;
            } else {
                app.cancel_input();
            }
        }
        KeyCode::Enter => {
            if app.completions_visible {
                app.apply_completion();
            }
            app.confirm_add_project()?;
        }
        KeyCode::Tab => {
            if app.completions_visible && app.completions.len() == 1 {
                app.apply_completion();
                app.update_completions();
            } else if app.completions_visible && app.completions.len() > 1 {
                app.complete_common_prefix();
            } else {
                app.update_completions();
            }
        }
        KeyCode::Up => {
            if app.completions_visible && !app.completions.is_empty() {
                let len = app.completions.len();
                app.completion_selected = (app.completion_selected + len - 1) % len;
            }
        }
        KeyCode::Down => {
            if app.completions_visible && !app.completions.is_empty() {
                let len = app.completions.len();
                app.completion_selected = (app.completion_selected + 1) % len;
            }
        }
        KeyCode::Backspace => {
            if app.input_cursor > 0 {
                app.input_cursor -= 1;
                app.input_buffer.remove(app.input_cursor);
                app.update_completions();
            }
        }
        KeyCode::Left => {
            if app.input_cursor > 0 {
                app.input_cursor -= 1;
            }
        }
        KeyCode::Right => {
            if app.input_cursor < app.input_buffer.len() {
                app.input_cursor += 1;
            }
        }
        KeyCode::Char(c) => {
            app.input_buffer.insert(app.input_cursor, c);
            app.input_cursor += 1;
            app.update_completions();
        }
        _ => {}
    }
    Ok(())
}

/// Handle keys in the fuzzy picker overlay.
pub(super) fn handle_fuzzy_picker_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.cancel_fuzzy_picker();
        }
        KeyCode::Enter => {
            if let Some(ref _state) = app.fuzzy_picker {
                if _state.selected_path().is_some() {
                    app.confirm_fuzzy_add_project()?;
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.move_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.move_down();
            }
        }
        KeyCode::Up => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.move_up();
            }
        }
        KeyCode::Down => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.move_down();
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.backspace();
                state.tick();
            }
        }
        KeyCode::Left => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.cursor_left();
            }
        }
        KeyCode::Right => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.cursor_right();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.insert_char(c);
                state.tick();
            }
        }
        _ => {}
    }
    Ok(())
}
