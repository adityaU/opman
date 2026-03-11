use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;

pub(super) fn handle_context_input_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.context_input = None;
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Submit: send text as a system message to the active OpenCode session
            if let Some(state) = app.context_input.take() {
                let text = state.to_string();
                if !text.trim().is_empty() {
                    if let Some(project) = app.projects.get(app.active_project) {
                        if let Some(ref session_id) = project.active_session {
                            let proj_dir = project.path.to_string_lossy().to_string();
                            let sid = session_id.clone();
                            let base_url = crate::app::base_url().to_string();
                            tracing::info!(
                                session_id = sid,
                                "Sending context input as system message"
                            );
                            tokio::spawn(async move {
                                let client = crate::api::ApiClient::new();
                                let msg = format!("[SYSTEM CONTEXT from user] {text}");
                                match client
                                    .send_system_message_async(&base_url, &proj_dir, &sid, &msg)
                                    .await
                                {
                                    Ok(()) => {
                                        tracing::info!("Context system message sent successfully")
                                    }
                                    Err(e) => tracing::error!(
                                        "Failed to send context system message: {e}"
                                    ),
                                }
                            });
                        }
                    }
                }
            }
        }
        KeyCode::Enter => {
            if let Some(ref mut state) = app.context_input {
                state.insert_newline();
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut state) = app.context_input {
                state.backspace();
            }
        }
        KeyCode::Left => {
            if let Some(ref mut state) = app.context_input {
                state.cursor_left();
            }
        }
        KeyCode::Right => {
            if let Some(ref mut state) = app.context_input {
                state.cursor_right();
            }
        }
        KeyCode::Up => {
            if let Some(ref mut state) = app.context_input {
                state.cursor_up();
            }
        }
        KeyCode::Down => {
            if let Some(ref mut state) = app.context_input {
                state.cursor_down();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut state) = app.context_input {
                state.insert_char(c);
            }
        }
        _ => {}
    }
    Ok(())
}
