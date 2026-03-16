//! Provider cache hook — singleton-cached provider/model data.
//! Matches React `useProviders.ts`.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::api::client::{api_fetch, ApiError};

// ── Types ──────────────────────────────────────────────────────────

/// A single model from a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub context_length: u64,
    #[serde(default)]
    pub supports_images: bool,
    #[serde(default)]
    pub supports_tools: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
}

/// A provider with its models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub models: HashMap<String, ModelInfo>,
}

/// Raw API response from /providers.
#[derive(Debug, Clone, Deserialize)]
struct ProvidersApiResponse {
    #[serde(default)]
    pub all: Vec<Provider>,
    #[serde(default)]
    pub connected: Vec<String>,
    #[serde(default, rename = "default")]
    pub defaults: HashMap<String, String>,
}

/// The provider cache returned by `use_providers`.
#[derive(Clone, Copy)]
pub struct ProviderCache {
    pub all: ReadSignal<Vec<Provider>>,
    pub connected: ReadSignal<HashSet<String>>,
    pub defaults: ReadSignal<HashMap<String, String>>,
    pub loading: ReadSignal<bool>,
    pub error: ReadSignal<Option<String>>,
    refresh: WriteSignal<u32>,
}

impl ProviderCache {
    /// Force a re-fetch of providers.
    pub fn refresh(&self) {
        self.refresh.update(|v| *v += 1);
    }

    /// Find a model by provider + model ID.
    pub fn find_model(&self, provider_id: &str, model_id: &str) -> Option<ModelInfo> {
        self.all
            .get_untracked()
            .iter()
            .find(|p| p.id == provider_id)
            .and_then(|p| p.models.get(model_id).cloned())
    }

    /// Get context length for a provider/model pair.
    pub fn context_limit(&self, provider_id: &str, model_id: &str) -> Option<u64> {
        self.find_model(provider_id, model_id)
            .map(|m| m.context_length)
    }
}

// ── Hook ───────────────────────────────────────────────────────────

/// Create the provider cache. Call once at layout level.
pub fn use_providers() -> ProviderCache {
    let (all, set_all) = signal(Vec::<Provider>::new());
    let (connected, set_connected) = signal(HashSet::<String>::new());
    let (defaults, set_defaults) = signal(HashMap::<String, String>::new());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (refresh_trigger, set_refresh_trigger) = signal(0u32);

    // Fetch on mount and on refresh trigger
    Effect::new(move |_| {
        let _trigger = refresh_trigger.get(); // track
        set_loading.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            match api_fetch::<ProvidersApiResponse>("/providers").await {
                Ok(resp) => {
                    set_all.set(resp.all);
                    set_connected.set(resp.connected.into_iter().collect());
                    set_defaults.set(resp.defaults);
                }
                Err(e) => {
                    set_error.set(Some(e.message));
                }
            }
            set_loading.set(false);
        });
    });

    ProviderCache {
        all,
        connected,
        defaults,
        loading,
        error,
        refresh: set_refresh_trigger,
    }
}
