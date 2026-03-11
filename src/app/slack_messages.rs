use crate::app::App;
use tracing::{error, info, warn};

impl App {
    /// Route a triaged message to a free session in the target project,
    /// or report that all sessions are busy.
    pub(super) fn handle_triage_route_to_project(
        &mut self,
        thread_ts: &str,
        channel: &str,
        original_text: &str,
        rewritten_query: &Option<String>,
        path: &str,
        connect_only: bool,
    ) {
        let project_idx = self
            .projects
            .iter()
            .position(|p| p.path.to_string_lossy() == path);

        if let Some(pidx) = project_idx {
            let project = &self.projects[pidx];
            let idle_minutes = self.config.settings.slack.idle_session_minutes;

            info!(
                "Slack triage: target project \"{}\" (idx={}) has {} sessions, {} active_sessions total",
                project.name,
                pidx,
                project.sessions.len(),
                self.active_sessions.len(),
            );

            if let Some(session_id) = crate::slack::find_free_session(
                &project.sessions,
                &self.active_sessions,
                idle_minutes,
            ) {
                info!(
                    "Slack: routing message to session {} in project {}",
                    session_id, project.name
                );

                let base_url = crate::app::base_url().to_string();
                let project_dir = project.path.to_string_lossy().to_string();
                let sid = session_id.clone();
                let text = rewritten_query
                    .clone()
                    .unwrap_or_else(|| original_text.to_string());
                let bot_token = self
                    .slack_auth
                    .as_ref()
                    .map(|a| a.bot_token.clone())
                    .unwrap_or_default();
                let ch = channel.to_string();
                let tts = thread_ts.to_string();
                let pname = project.name.clone();
                let sname = project
                    .sessions
                    .iter()
                    .find(|s| s.id == session_id)
                    .map(|s| s.title.clone())
                    .unwrap_or_else(|| session_id.clone());
                let slack_state = self.slack_state.clone();
                let buffer_secs = self.config.settings.slack.relay_buffer_secs;
                let is_connect_only = connect_only;

                tokio::spawn(async move {
                    let client = reqwest::Client::new();

                    // Detach any previous relay from this thread, then record new mapping.
                    if let Some(ref st) = slack_state {
                        let mut s = st.lock().await;
                        if let Some((old_ts, _)) = s.detach_relay_by_session(&sid) {
                            tracing::info!(
                                "Slack triage: detached relay for session {} from old thread {}",
                                &sid[..8.min(sid.len())],
                                old_ts
                            );
                        }
                        if let Some(old_sid) = s.detach_relay(&tts) {
                            tracing::info!(
                                "Slack triage: detached previous relay (session {}) from thread {}",
                                &old_sid[..8.min(old_sid.len())],
                                tts
                            );
                        }
                        s.thread_sessions.insert(tts.clone(), (pidx, sid.clone()));
                        s.session_threads
                            .insert(sid.clone(), (ch.clone(), tts.clone()));
                        s.active_relay.insert(tts.clone(), sid.clone());
                        s.metrics.messages_routed += 1;
                        s.metrics.last_routed_at = Some(std::time::Instant::now());
                        s.log(
                            crate::slack::SlackLogLevel::Info,
                            format!(
                                "Routed message to project \"{}\" session {}",
                                pname,
                                &sid[..8.min(sid.len())]
                            ),
                        );
                    }

                    // Record current message offset.
                    if let Some(ref st) = slack_state {
                        match crate::slack::fetch_session_messages_with_tools(
                            &client,
                            &base_url,
                            &project_dir,
                            &sid,
                        )
                        .await
                        {
                            Ok(msgs) => {
                                let mut s = st.lock().await;
                                s.session_msg_offset.insert(sid.clone(), msgs.len());
                                tracing::debug!(
                                    "Slack: recorded msg offset {} for session {}",
                                    msgs.len(),
                                    sid
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Slack: failed to fetch msg offset for triage result: {}",
                                    e
                                );
                            }
                        }
                    }

                    if is_connect_only {
                        info!(
                            "Slack: connected thread to session {} (connect-only, no message forwarded)",
                            sid
                        );
                        let ack = format!(
                            "connected to project: {}, session: {} — send a message in this thread to start working",
                            pname, sname
                        );
                        let _ = crate::slack::post_message(
                            &client, &bot_token, &ch, &ack, Some(&tts),
                        )
                        .await;
                    } else {
                        match crate::slack::send_user_message(
                            &client,
                            &base_url,
                            &project_dir,
                            &sid,
                            &text,
                        )
                        .await
                        {
                            Ok(()) => {
                                info!("Slack: user message sent to session {}", sid);
                                let ack = format!(
                                    "relayed to project: {}, session: {}",
                                    pname, sname
                                );
                                let _ = crate::slack::post_message(
                                    &client, &bot_token, &ch, &ack, Some(&tts),
                                )
                                .await;
                            }
                            Err(e) => {
                                error!("Failed to send message to session: {}", e);
                                let msg = format!("opman: Failed to send message: {}", e);
                                let _ = crate::slack::post_message(
                                    &client, &bot_token, &ch, &msg, Some(&tts),
                                )
                                .await;
                            }
                        }
                    }

                    // Spawn a live relay watcher for this session.
                    if let Some(ref st) = slack_state {
                        let already_watching = {
                            let s = st.lock().await;
                            s.relay_abort_handles.contains_key(&sid)
                        };
                        if !already_watching {
                            let handle = crate::slack::spawn_session_relay_watcher(
                                sid.clone(),
                                project_dir.clone(),
                                ch.clone(),
                                tts.clone(),
                                bot_token.clone(),
                                base_url.clone(),
                                buffer_secs,
                                st.clone(),
                            );
                            let mut s = st.lock().await;
                            s.relay_abort_handles
                                .insert(sid.clone(), handle.abort_handle());
                            let map = crate::slack::SlackSessionMap {
                                session_threads: s.session_threads.clone(),
                                thread_sessions: s.thread_sessions.clone(),
                                msg_offsets: s.session_msg_offset.clone(),
                            };
                            if let Err(e) = map.save() {
                                tracing::warn!("Slack: failed to persist session map: {}", e);
                            }
                        }
                    }
                });
            } else {
                // No free session — tell the user.
                warn!(
                    "Slack: no free session in project {} — user must request creation explicitly",
                    project.name
                );
                if let Some(ref auth) = self.slack_auth {
                    let bot_token = auth.bot_token.clone();
                    let ch = channel.to_string();
                    let tts = thread_ts.to_string();
                    let pname = project.name.clone();
                    let session_count = project.sessions.len();
                    tokio::spawn(async move {
                        let client = reqwest::Client::new();
                        let msg = format!(
                            "opman: All {} session(s) in *{}* are currently busy. \
                             To create a new session, say something like \
                             \"create a new session in {}\" or \"start a fresh session for {}\".",
                            session_count, pname, pname, pname
                        );
                        let _ = crate::slack::post_message(
                            &client, &bot_token, &ch, &msg, Some(&tts),
                        )
                        .await;
                    });
                }
            }
        } else {
            warn!("Slack: no project found at path {:?}", path);
            self.post_project_not_found_error(channel, thread_ts, path);
        }
    }
}
