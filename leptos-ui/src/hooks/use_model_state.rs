//! Model state hook — selected model, agent, derived current model.
//! Matches React `useModelState.ts`.

use leptos::prelude::*;

use crate::hooks::use_providers::ProviderCache;
use crate::types::core::Message;

// ── Types ──────────────────────────────────────────────────────────

/// A reference to a specific model.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelRef {
    pub provider_id: String,
    pub model_id: String,
}

/// Cached latest-assistant model info, derived from messages.
/// Updated once per session or when messages change — but kept as a separate
/// signal so `current_model` / `current_model_context_limit` memos can read
/// a tiny signal instead of the full messages Vec during streaming.
#[derive(Debug, Clone, PartialEq)]
struct LatestAssistantModel {
    model_id: Option<String>,
    provider_id: Option<String>,
}

/// Model state returned by `use_model_state`.
#[derive(Clone, Copy)]
pub struct ModelState {
    pub selected_model: ReadSignal<Option<ModelRef>>,
    pub set_selected_model: WriteSignal<Option<ModelRef>>,
    pub selected_agent: ReadSignal<String>,
    pub set_selected_agent: WriteSignal<String>,
    pub sending: ReadSignal<bool>,
    pub set_sending: WriteSignal<bool>,
    /// Derived: current model ID from latest assistant message or selection.
    pub current_model: Memo<Option<String>>,
    /// Derived: default model display string for new sessions.
    pub default_model_display: Memo<Option<String>>,
    /// Derived: context limit for the active model.
    pub current_model_context_limit: Memo<Option<u64>>,
}

// ── Hook ───────────────────────────────────────────────────────────

/// Create model state. Call once at layout level.
pub fn use_model_state(messages: ReadSignal<Vec<Message>>, providers: ProviderCache) -> ModelState {
    let (selected_model, set_selected_model) = signal(Option::<ModelRef>::None);
    let (selected_agent, set_selected_agent) = signal(String::from("coder"));
    let (sending, set_sending) = signal(false);

    // Cache latest assistant model info as a small derived memo.
    // This reads `messages` but only produces a tiny PartialEq value,
    // so downstream memos won't re-run unless the model actually changes.
    let latest_assistant = Memo::new(move |_| {
        let msgs = messages.get();
        let latest = msgs
            .iter()
            .rev()
            .find(|m| m.info.role == "assistant" && m.info.model_id.is_some());
        LatestAssistantModel {
            model_id: latest.and_then(|m| m.info.model_id.clone()),
            provider_id: latest.and_then(|m| m.info.provider_id.clone()),
        }
    });

    // Current model: from explicit selection, or from cached latest assistant model
    let current_model = Memo::new(move |_| {
        if let Some(ref sel) = selected_model.get() {
            return Some(sel.model_id.clone());
        }
        latest_assistant.get().model_id
    });

    // Default model display: first provider default
    let providers_all = providers.all;
    let providers_defaults = providers.defaults;
    let default_model_display = Memo::new(move |_| {
        let defaults = providers_defaults.get();
        let all = providers_all.get();

        // Find first connected provider's default
        if let Some((provider_id, model_id)) = defaults.iter().next() {
            if let Some(provider) = all.iter().find(|p| &p.id == provider_id) {
                if let Some(model) = provider.models.get(model_id) {
                    return Some(model.name.clone().unwrap_or_else(|| model_id.clone()));
                }
                return Some(model_id.clone());
            }
            return Some(format!("{}/{}", provider_id, model_id));
        }
        None
    });

    // Context limit for current model
    let current_model_context_limit = Memo::new(move |_| {
        let sel = selected_model.get();
        if let Some(ref model_ref) = sel {
            return providers.context_limit(&model_ref.provider_id, &model_ref.model_id);
        }
        // Fallback: from cached latest assistant model
        let cached = latest_assistant.get();
        if let (Some(ref pid), Some(ref mid)) = (&cached.provider_id, &cached.model_id) {
            return providers.context_limit(pid, mid);
        }
        None
    });

    ModelState {
        selected_model,
        set_selected_model,
        selected_agent,
        set_selected_agent,
        sending,
        set_sending,
        current_model,
        default_model_display,
        current_model_context_limit,
    }
}
