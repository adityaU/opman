//! SSE + Slack integration helpers for `App::handle_background_event`.
//!
//! Contains: `setup_subagent_slack_thread`, `handle_sse_message_updated`,
//! `handle_sse_todo_updated`, `handle_sse_permission_asked`, `handle_sse_question_asked`.

use tracing::{debug, info};

use crate::app::{App, PermissionRequest, QuestionRequest, SessionStats, TodoItem};

impl App {
    /// Async helper: set up a Slack thread for a subagent session.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn setup_subagent_slack_thread(
        project_idx: usize,
        child_sid: String,
        child_title: String,
        parent_sid: String,
        bot_token: String,
        base_url: String,
        project_dir: String,
        buffer_secs: u64,
        parent_channel: String,
        parent_thread_ts: String,
        st: std::sync::Arc<tokio::sync::Mutex<crate::slack::SlackState>>,
    ) {
        let client = reqwest::Client::new();
        let parent_link = crate::slack::slack_thread_link(&parent_channel, &parent_thread_ts);
        let title_display = if child_title.is_empty() { "Subagent".to_string() } else { child_title.clone() };

        let top_msg = format!(":robot_face: *Subagent:* {}\n:link: Parent thread: {}", title_display, parent_link);
        let subagent_ts = match crate::slack::post_message(&client, &bot_token, &parent_channel, &top_msg, None).await {
            Ok(ts) => ts,
            Err(e) => {
                tracing::warn!("Slack subagent thread: failed to post top-level message: {}", e);
                return;
            }
        };
        tracing::info!(
            "Slack subagent thread: created {} for child session {} (parent {})",
            subagent_ts, &child_sid[..8.min(child_sid.len())], &parent_sid[..8.min(parent_sid.len())]
        );

        let child_link = crate::slack::slack_thread_link(&parent_channel, &subagent_ts);
        let parent_notify = format!(":robot_face: Subagent started: {} → {}", title_display, child_link);
        let _ = crate::slack::post_message(&client, &bot_token, &parent_channel, &parent_notify, Some(&parent_thread_ts)).await;

        {
            let mut s = st.lock().await;
            s.thread_sessions.insert(subagent_ts.clone(), (project_idx, child_sid.clone()));
            s.session_threads.insert(child_sid.clone(), (parent_channel.clone(), subagent_ts.clone()));
            s.subagent_threads.insert(child_sid.clone(), (parent_channel.clone(), subagent_ts.clone(), parent_sid.clone()));
        }

        match crate::slack::fetch_session_messages_with_tools(&client, &base_url, &project_dir, &child_sid).await {
            Ok(msgs) => { st.lock().await.session_msg_offset.insert(child_sid.clone(), msgs.len()); }
            Err(_) => { st.lock().await.session_msg_offset.insert(child_sid.clone(), 0); }
        }

        let handle = crate::slack::spawn_session_relay_watcher(
            child_sid.clone(), project_dir.clone(), parent_channel.clone(),
            subagent_ts.clone(), bot_token.clone(), base_url.clone(), buffer_secs, st.clone(),
        );
        {
            let mut s = st.lock().await;
            s.relay_abort_handles.insert(child_sid.clone(), handle.abort_handle());
            let map = crate::slack::SlackSessionMap {
                session_threads: s.session_threads.clone(),
                thread_sessions: s.thread_sessions.clone(),
                msg_offsets: s.session_msg_offset.clone(),
            };
            if let Err(e) = map.save() {
                tracing::warn!("Slack subagent thread: failed to persist session map: {}", e);
            }
        }
    }

    /// Handle `BackgroundEvent::SseMessageUpdated`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_sse_message_updated(
        &mut self,
        session_id: String,
        cost: f64,
        input_tokens: u64,
        output_tokens: u64,
        reasoning_tokens: u64,
        cache_read: u64,
        cache_write: u64,
    ) {
        let stats = self.session_stats.entry(session_id.clone()).or_insert_with(SessionStats::default);
        stats.prev_total_tokens = stats.total_tokens();
        stats.cost = cost;
        stats.input_tokens = input_tokens;
        stats.output_tokens = output_tokens;
        stats.reasoning_tokens = reasoning_tokens;
        stats.cache_read = cache_read;
        stats.cache_write = cache_write;
        self.last_message_event_at.insert(session_id.clone(), std::time::Instant::now());
        debug!(session_id, cost, input_tokens, output_tokens, "SSE: message.updated with token/cost data");
        if let Some(ss) = self.slack_state.clone() {
            let sid = session_id.clone();
            tokio::spawn(async move { ss.lock().await.notify_relay(&sid); });
        }
    }

    /// Handle `BackgroundEvent::SseTodoUpdated`.
    pub(crate) fn handle_sse_todo_updated(&mut self, session_id: String, todos: Vec<TodoItem>) {
        debug!(session_id, count = todos.len(), "SSE todo.updated");
        if let Some(ref mut panel) = self.todo_panel {
            if panel.session_id == session_id {
                panel.todos = todos.clone();
                if panel.selected >= panel.todos.len() {
                    panel.selected = panel.todos.len().saturating_sub(1);
                }
            }
        }
        if let (Some(ref ss), Some(ref auth)) = (self.slack_state.clone(), self.slack_auth.clone()) {
            let ss = ss.clone();
            let bot_token = auth.bot_token.clone();
            let sid = session_id.clone();
            tokio::spawn(async move {
                let (channel, thread_ts, existing_ts) = {
                    let guard = ss.lock().await;
                    let thread_info = guard.session_threads.get(&sid).cloned();
                    let existing = guard.todo_message_ts.get(&sid).cloned();
                    match thread_info {
                        Some((ch, ts)) => (ch, ts, existing),
                        None => return,
                    }
                };
                let text = crate::slack::render_todos_mrkdwn(&todos);
                let client = reqwest::Client::new();
                if let Some(ref msg_ts) = existing_ts {
                    if let Err(e) = crate::slack::update_message(&client, &bot_token, &channel, msg_ts, &text).await {
                        tracing::warn!("Slack todo update failed for session {}: {}", &sid[..8.min(sid.len())], e);
                    }
                } else {
                    match crate::slack::post_message(&client, &bot_token, &channel, &text, Some(&thread_ts)).await {
                        Ok(ts) => {
                            let mut guard = ss.lock().await;
                            guard.todo_message_ts.insert(sid.clone(), ts);
                            tracing::info!("Slack todo: posted checklist for session {}", &sid[..8.min(sid.len())]);
                        }
                        Err(e) => {
                            tracing::warn!("Slack todo post failed for session {}: {}", &sid[..8.min(sid.len())], e);
                        }
                    }
                }
            });
        }
    }

    /// Handle `BackgroundEvent::SsePermissionAsked`.
    pub(crate) fn handle_sse_permission_asked(&mut self, project_idx: usize, request: PermissionRequest) {
        info!(
            project_idx, request_id = %request.id,
            session_id = %request.session_id, permission = %request.permission,
            "Permission request received from AI agent"
        );
        // Track that this session needs input
        if !request.session_id.is_empty() {
            self.input_sessions.insert(request.session_id.clone());
        }
        if let (Some(ref ss), Some(ref auth)) = (self.slack_state.clone(), self.slack_auth.clone()) {
            let ss = ss.clone();
            let bot_token = auth.bot_token.clone();
            let sid = request.session_id.clone();
            let req = request.clone();
            let pidx = project_idx;
            tokio::spawn(async move {
                let (channel, thread_ts) = {
                    let guard = ss.lock().await;
                    match guard.session_threads.get(&sid).cloned() {
                        Some((ch, ts)) => (ch, ts),
                        None => return,
                    }
                };
                let (fallback, blocks) = crate::slack::render_permission_blocks(&req);
                let client = reqwest::Client::new();
                match crate::slack::post_message_with_blocks(
                    &client, &bot_token, &channel, &fallback, &blocks, None, Some(&thread_ts),
                ).await {
                    Ok(msg_ts) => {
                        let mut guard = ss.lock().await;
                        guard.pending_permissions.insert(
                            thread_ts.clone(),
                            (req.id.clone(), sid.clone(), pidx, msg_ts, req.clone()),
                        );
                        tracing::info!(
                            "Slack: posted permission request {} for session {}",
                            &req.id[..8.min(req.id.len())], &sid[..8.min(sid.len())]
                        );
                    }
                    Err(e) => tracing::warn!("Slack: failed to post permission request: {}", e),
                }
            });
        }
    }

    /// Handle `BackgroundEvent::SseQuestionAsked`.
    pub(crate) fn handle_sse_question_asked(&mut self, project_idx: usize, request: QuestionRequest) {
        info!(
            project_idx, request_id = %request.id,
            session_id = %request.session_id, question_count = request.questions.len(),
            "Question request received from AI agent"
        );
        // Track that this session needs input
        if !request.session_id.is_empty() {
            self.input_sessions.insert(request.session_id.clone());
        }
        if let (Some(ref ss), Some(ref auth)) = (self.slack_state.clone(), self.slack_auth.clone()) {
            let ss = ss.clone();
            let bot_token = auth.bot_token.clone();
            let sid = request.session_id.clone();
            let req = request.clone();
            let pidx = project_idx;
            tokio::spawn(async move {
                let (channel, thread_ts) = {
                    let guard = ss.lock().await;
                    match guard.session_threads.get(&sid).cloned() {
                        Some((ch, ts)) => (ch, ts),
                        None => return,
                    }
                };
                let (fallback, blocks) = crate::slack::render_question_blocks(&req);
                let client = reqwest::Client::new();
                match crate::slack::post_message_with_blocks(
                    &client, &bot_token, &channel, &fallback, &blocks, None, Some(&thread_ts),
                ).await {
                    Ok(msg_ts) => {
                        let mut guard = ss.lock().await;
                        guard.pending_questions.insert(
                            thread_ts.clone(),
                            (req.id.clone(), sid.clone(), pidx, msg_ts, req.clone()),
                        );
                        tracing::info!(
                            "Slack: posted question {} for session {}",
                            &req.id[..8.min(req.id.len())], &sid[..8.min(sid.len())]
                        );
                    }
                    Err(e) => tracing::warn!("Slack: failed to post question: {}", e),
                }
            });
        }
    }
}
