//! Keep write-only settings and sensitive upstream errors out of AI responses.
use serde_json::Value;

fn sensitive(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace(['_', '-'], "");
    matches!(
        key.as_str(),
        "env"
            | "headers"
            | "password"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "clientsecret"
            | "secret"
            | "apikey"
            | "authorization"
            | "lasterror"
    )
}

pub(super) fn redact(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            fields.retain(|key, _| !sensitive(key));
            for (name, field) in fields {
                if matches!(name.as_str(), "inputSchema" | "outputSchema") {
                    continue;
                }
                if name == "args" {
                    redact_args(field);
                }
                redact(field);
            }
        }
        Value::Array(values) => {
            for value in values {
                redact(value);
            }
        }
        Value::String(text) => {
            let Ok(mut url) = url::Url::parse(text) else {
                return;
            };
            if !matches!(url.scheme(), "http" | "https") {
                return;
            }
            let mut changed = false;
            if !url.username().is_empty() || url.password().is_some() {
                let _ = url.set_username("");
                let _ = url.set_password(None);
                changed = true;
            }
            let has_secret = url.query_pairs().any(|(key, _)| query_sensitive(&key));
            if has_secret {
                let safe: Vec<_> = url
                    .query_pairs()
                    .filter(|(key, _)| !query_sensitive(key))
                    .map(|(key, value)| (key.into_owned(), value.into_owned()))
                    .collect();
                url.set_query(None);
                if !safe.is_empty() {
                    url.query_pairs_mut().extend_pairs(safe);
                }
                changed = true;
            }
            if changed {
                *text = url.into();
            }
        }
        _ => {}
    }
}

fn query_sensitive(key: &str) -> bool {
    sensitive(key)
        || matches!(
            key.to_ascii_lowercase().as_str(),
            "key" | "auth" | "credential" | "credentials"
        )
}

fn redact_args(value: &mut Value) {
    let Some(args) = value.as_array_mut() else {
        return;
    };
    let mut hide_next = false;
    for arg in args {
        let Some(text) = arg.as_str() else {
            continue;
        };
        if hide_next {
            *arg = Value::String("<redacted>".into());
            hide_next = false;
            continue;
        }
        let (flag, inline) = text
            .split_once('=')
            .map_or((text, None), |(flag, value)| (flag, Some(value)));
        let key = flag.trim_start_matches('-');
        if !flag.starts_with('-')
            || !(query_sensitive(key) || matches!(key, "H" | "header" | "headers"))
        {
            continue;
        }
        if inline.is_some() {
            *arg = Value::String(format!("{flag}=<redacted>"));
        } else {
            hide_next = true;
        }
    }
}

pub(super) fn section(name: &str) -> &'static str {
    let name = name.strip_prefix("settings_").unwrap_or(name);
    match name.split('_').next() {
        Some("theme" | "themes" | "appearance") => "appearance",
        Some("keybindings") => "keybindings",
        Some("server" | "servers") => "servers",
        Some("project" | "projects") => "projects",
        Some("agent" | "agents") => "acp",
        Some("runner" | "runners") if name.starts_with("runner_limits") => "limits",
        Some("runner" | "runners") => "acp",
        Some("mcp") => "mcp",
        Some("skill" | "skills") => "skills",
        _ => "all",
    }
}
