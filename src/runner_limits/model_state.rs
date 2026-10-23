//! Read the current model from the shapes returned by runner transcripts.

use serde_json::Value;

use super::settings::ModelRef;

pub(super) fn model_ref(value: Option<&Value>) -> Option<ModelRef> {
    let value = value?;
    if let Some(id) = value.as_str().filter(|id| !id.is_empty()) {
        return Some(ModelRef {
            provider_id: String::new(),
            model_id: id.into(),
        });
    }
    Some(ModelRef {
        provider_id: value.get("providerID")?.as_str()?.into(),
        model_id: value.get("modelID")?.as_str()?.into(),
    })
}

pub fn session_model(history: &Value) -> Option<ModelRef> {
    history.as_array()?.iter().rev().find_map(|message| {
        let info = message.get("info")?;
        let mut model = model_ref(info.get("model")).or_else(|| model_ref(Some(info)))?;
        if model.provider_id.is_empty() {
            if let Some(provider) = info.get("providerID").and_then(Value::as_str) {
                model.provider_id = provider.into();
            }
        }
        Some(model)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn latest_model_handles_opencode_and_acp_metadata() {
        for (history, provider, model) in [
            (
                json!([{"info":{"providerID":"anthropic","modelID":"opus"}}]),
                "anthropic",
                "opus",
            ),
            (
                json!([{"info":{"model":{"providerID":"codex","modelID":"sol"}}}]),
                "codex",
                "sol",
            ),
            (
                json!([{"info":{"model":"sol","providerID":"openai"}}]),
                "openai",
                "sol",
            ),
            (
                json!([{"info":{"model":"opus"}}, {"info":{"model":"sonnet"}}, {"info":{"role":"user"}}]),
                "",
                "sonnet",
            ),
        ] {
            let actual = session_model(&history);
            assert_eq!(
                actual,
                Some(ModelRef {
                    provider_id: provider.into(),
                    model_id: model.into()
                })
            );
        }
    }
}
