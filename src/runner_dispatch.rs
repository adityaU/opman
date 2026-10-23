//! Send routing and stopping the source before a runner handoff.
use super::*;

#[path = "runner_handoff_guard.rs"]
pub(super) mod guard;

#[cfg(test)]
#[path = "runner_dispatch_tests.rs"]
mod tests;

impl RunnerRegistry {
    pub async fn send_message(
        &self,
        session_id: &str,
        directory: &str,
        requested: Option<RunnerKind>,
        body: Value,
    ) -> Result<SendOutcome> {
        // A session an automatic switch moved on is answered where it continues, so a
        // caller still holding the old id (kanban, the agent-manager) does not fork it again.
        let forwarded = match requested {
            Some(_) => None,
            None => crate::runner_limits::watch::global().forward_of(session_id),
        };
        let moved = forwarded.is_some();
        let session_id = forwarded.as_deref().unwrap_or(session_id);
        let (session_id, directory) = validate_location(session_id, directory)?;
        let logical_session_id = session_id.to_string();
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        let current = self.binding(&logical_session_id, directory).await;
        // A send that names no runner goes elsewhere when the user asked opman to move
        // sessions off a runner that is running low.
        let mut body = body;
        let redirected = match requested {
            Some(_) => None,
            None => {
                let settings = crate::runner_limits::settings::global().get();
                let saved_model = if body.get("model").is_none()
                    && settings.policy == crate::runner_limits::settings::LimitPolicy::Auto
                    && !settings.model_sets.is_empty()
                {
                    self.handoff_model(&current, &logical_session_id, directory)
                        .await
                } else {
                    None
                };
                crate::runner_limits::watch::redirect_with_model(
                    &current.runner,
                    &self.available(),
                    &mut body,
                    saved_model.as_ref(),
                )
            }
        };
        let target_kind = requested
            .or(redirected)
            .unwrap_or_else(|| current.runner.clone());
        let target = self
            .runner(&target_kind)
            .context("requested runner is not available")?;
        // Refuse before anything is read or created: a handoff into a disabled runner would
        // otherwise read the whole transcript only to fail at the last step.
        if target.is_disabled() {
            return Err(RunnerDisabled::new(target_kind).into());
        }
        if target_kind == current.runner {
            let response = target
                .send_message(&current.physical_id, directory, body)
                .await?;
            return Ok(SendOutcome {
                session_id: logical_session_id,
                runner: target_kind,
                switched: moved,
                response,
            });
        }

        let _handoff = self
            .handoff_guards
            .acquire(directory, &logical_session_id)?;
        if let Some(moved_to) =
            crate::runner_limits::watch::global().forward_of(&logical_session_id)
        {
            if self.has_binding(&moved_to).await {
                bail!(
                    "This chat already moved to session {moved_to}. Open that session to continue."
                );
            }
        }
        let source = self
            .runner(&current.runner)
            .context("current runner is not available")?;
        source
            .abort(&current.physical_id, &current.directory)
            .await
            .context("could not stop the previous runner before handoff")?;
        let history = source
            .messages(&current.physical_id, &current.directory)
            .await?;
        let transcript = crate::runner_handoff::render_transcript(&history);
        let user_text = extract_text(&body);
        let handoff = crate::runner_handoff::build_prompt(
            &current.runner.display_name(),
            &transcript,
            &user_text,
        );
        let session = target.create_session(directory, "Handoff session").await?;
        let mut handoff_body = body;
        handoff_body["parts"] = json!([{ "type": "text", "text": handoff }]);
        self.carry_permission(&target_kind, directory, &session.id, &mut handoff_body)
            .await;
        let response = target
            .send_message(&session.id, directory, handoff_body)
            .await?;
        self.bindings.write().await.insert(
            session.id.clone(),
            Binding {
                runner: target_kind.clone(),
                physical_id: session.id.clone(),
                directory: directory.to_string(),
            },
        );
        crate::runner_limits::watch::global().record_forward(&logical_session_id, &session.id);
        Ok(SendOutcome {
            session_id: session.id,
            runner: target_kind,
            switched: true,
            response,
        })
    }

    /// Model metadata is optional: failing to read it must not block an ordinary send.
    async fn handoff_model(
        &self,
        current: &Binding,
        session_id: &str,
        directory: &str,
    ) -> Option<crate::runner_limits::settings::ModelRef> {
        if let Some(model) = self
            .session_engine(session_id, directory)
            .await
            .ok()
            .and_then(|engine| engine.model)
        {
            return Some(crate::runner_limits::settings::ModelRef {
                provider_id: String::new(),
                model_id: model.as_str().to_string(),
            });
        }
        let source = self.runner(&current.runner).ok()?;
        let history = source
            .messages(&current.physical_id, &current.directory)
            .await
            .ok()?;
        crate::runner_limits::watch::session_model(&history)
    }

    /// Carry the composer's permission mode onto a freshly created handoff session.
    ///
    /// A mode is a name in one runner's vocabulary — `bypassPermissions` means nothing to
    /// codex — so one the target does not publish is dropped rather than forwarded: an
    /// unrecognised mode is stored, pushed and silently ignored, and the handed-off session
    /// spends the conversation asking for permissions the user already granted. One the
    /// target does publish is also recorded on the session, so it outlives the first turn
    /// the way it would have on the runner the user chose it on.
    async fn carry_permission(
        &self,
        kind: &RunnerKind,
        directory: &str,
        session_id: &str,
        body: &mut Value,
    ) {
        let Some(object) = body.as_object_mut() else {
            return;
        };
        let Some(requested) = object.get("permission").and_then(Value::as_str) else {
            return;
        };
        let offered = self.permission_modes(kind.clone(), directory).await;
        if !offered.is_empty() && !offered.iter().any(|mode| mode == requested) {
            object.remove("permission");
            return;
        }
        let choices = crate::app::EngineChoices::from_parts(None, None, None, Some(requested));
        if let Ok(runner) = self.runner(kind) {
            let _ = runner.configure(session_id, directory, &choices).await;
        }
    }
}
