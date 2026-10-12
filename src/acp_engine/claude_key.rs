//! Whether the Claude agent should see `ANTHROPIC_API_KEY`.
//!
//! Interactive Claude Code asks once whether to use a key it finds in the environment and
//! records the answer in `~/.claude.json` under `customApiKeyResponses`, keyed by the key's
//! last 20 characters. The headless agent opman runs never asks: any key wins over the
//! user's subscription login, so a key exported for other tools ends up billed ("Credit
//! balance is too low"). opman applies interactive Claude's rule instead.

use std::path::PathBuf;

use serde_json::Value;

pub(super) const API_KEY_ENV: &str = "ANTHROPIC_API_KEY";

/// How much of a key Claude stores to recognise it later.
const KEY_TAIL: usize = 20;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum KeyUse {
    Keep,
    Strip,
}

/// True when the key in opman's environment should be kept from the Claude agent.
pub(super) fn unwanted() -> bool {
    let Some(key) = std::env::var(API_KEY_ENV)
        .ok()
        .filter(|key| !key.is_empty())
    else {
        return false;
    };
    let config = config_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or(Value::Null);
    decide(&key, &config) == KeyUse::Strip
}

/// Claude's rule: an approved key is used; a rejected one is not; an unanswered key is not
/// used by someone logged in with a subscription. With no login at all the key is the only
/// way in, so it stays.
pub(super) fn decide(key: &str, config: &Value) -> KeyUse {
    // Character-safe: `slice(-20)` on Claude's side, never a split UTF-8 sequence here.
    let tail = key
        .char_indices()
        .rev()
        .nth(KEY_TAIL - 1)
        .map_or(key, |(start, _)| &key[start..]);
    let listed = |list: &str| {
        config
            .pointer(&format!("/customApiKeyResponses/{list}"))
            .and_then(Value::as_array)
            .is_some_and(|keys| keys.iter().any(|entry| entry.as_str() == Some(tail)))
    };
    if listed("approved") {
        return KeyUse::Keep;
    }
    if listed("rejected")
        || config
            .get("oauthAccount")
            .is_some_and(|account| !account.is_null())
    {
        return KeyUse::Strip;
    }
    KeyUse::Keep
}

/// `$CLAUDE_CONFIG_DIR/.claude.json`, else `~/.claude.json` — where Claude keeps it.
fn config_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir).join(".claude.json"));
    }
    dirs::home_dir().map(|home| home.join(".claude.json"))
}

#[cfg(test)]
#[path = "claude_key_tests.rs"]
mod claude_key_tests;
