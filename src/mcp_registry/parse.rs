//! Turning config strings into [`Arg`]s.
//!
//! Placeholders are resolved into structure once, at load, so binding a spec to a
//! session is a substitution rather than a re-scan of every string at every launch.

use super::spec::Arg;

const DIR: &str = "dir";
const SESSION: &str = "session";
const ENV_PREFIX: &str = "env:";

/// Parse `${dir}`, `${session}` and `${env:NAME}` out of one config string.
///
/// A bare `$`, and an unrecognised `${…}`, are kept as literal text: a config value
/// that happens to contain a dollar sign should reach the runner unchanged rather than
/// silently lose characters. `server` only names the entry in the warning.
pub(crate) fn arg(raw: &str, server: &str) -> Arg {
    let mut parts: Vec<Arg> = Vec::new();
    let mut literal = String::new();
    let mut rest = raw;

    while let Some(start) = rest.find("${") {
        let Some(len) = rest[start..].find('}') else {
            break; // Unterminated — the remainder is literal.
        };
        let token = &rest[start + 2..start + len];
        match token_arg(token) {
            Some(parsed) => {
                literal.push_str(&rest[..start]);
                if !literal.is_empty() {
                    parts.push(Arg::lit(std::mem::take(&mut literal)));
                }
                parts.push(parsed);
            }
            None => {
                tracing::warn!(
                    server,
                    token,
                    "unknown placeholder in mcp.json, kept literal"
                );
                literal.push_str(&rest[..start + len + 1]);
            }
        }
        rest = &rest[start + len + 1..];
    }

    literal.push_str(rest);
    if !literal.is_empty() {
        parts.push(Arg::lit(literal));
    }

    // A single part stays flat; only genuinely mixed text needs the nested form.
    if parts.len() > 1 {
        return Arg::Mixed(parts.into_boxed_slice());
    }
    parts.pop().unwrap_or_else(|| Arg::lit(""))
}

fn token_arg(token: &str) -> Option<Arg> {
    if token == DIR {
        return Some(Arg::Dir);
    }
    if token == SESSION {
        return Some(Arg::SessionId);
    }
    let name = token.strip_prefix(ENV_PREFIX)?;
    if name.is_empty() {
        return None;
    }
    Some(Arg::Env(name.into()))
}

/// Parse an ordered map of config strings into name/[`Arg`] pairs.
pub(crate) fn pairs<'a>(
    entries: impl Iterator<Item = (&'a String, &'a String)>,
    server: &str,
) -> Vec<(Box<str>, Arg)> {
    entries
        .map(|(name, value)| (name.as_str().into(), arg(value, server)))
        .collect()
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod parse_tests;
