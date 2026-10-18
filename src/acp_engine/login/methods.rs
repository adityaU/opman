//! The login methods an agent lists in its `initialize` reply, and picking one.

use serde_json::Value;

/// How a login is carried out.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// The agent logs in over ACP: opman sends `authenticate` with the method's id.
    Agent,
    /// The agent's own command, run in the user's terminal with these extras. The agent
    /// asks for something only a person at a keyboard can give.
    Terminal {
        args: Vec<String>,
        env: Vec<(String, String)>,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Method {
    pub id: String,
    pub name: String,
    pub kind: Kind,
}

/// Every usable method. An entry with no id names nothing `authenticate` could be sent.
pub(super) fn parse(init: &Value) -> Vec<Method> {
    let Some(list) = init.get("authMethods").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter().filter_map(method).collect()
}

fn method(entry: &Value) -> Option<Method> {
    let id = entry.get("id").and_then(Value::as_str).filter(|id| !id.is_empty())?;
    let name = entry.get("name").and_then(Value::as_str).unwrap_or(id);
    let terminal = entry.get("type").and_then(Value::as_str) == Some("terminal");
    let kind = if terminal { terminal_kind(entry) } else { Kind::Agent };
    Some(Method {
        id: id.to_string(),
        name: name.to_string(),
        kind,
    })
}

fn terminal_kind(entry: &Value) -> Kind {
    let args = entry
        .get("args")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|arg| arg.as_str().map(str::to_string))
        .collect();
    let env = entry
        .get("env")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_string())))
        .collect();
    Kind::Terminal { args, env }
}

/// What to do about choosing.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Choice<'a> {
    Use(&'a Method),
    /// More than one method and no preference: the user has to say.
    Ask,
}

/// The method asked for by id, or the only one there is.
pub(super) fn choose<'a>(methods: &'a [Method], wanted: Option<&str>) -> Result<Choice<'a>, String> {
    if methods.is_empty() {
        return Err("the agent lists no login methods; log in with its own CLI".to_string());
    }
    if let Some(wanted) = wanted {
        return match methods.iter().find(|m| m.id == wanted) {
            Some(found) => Ok(Choice::Use(found)),
            None => {
                let known: Vec<&str> = methods.iter().map(|m| m.id.as_str()).collect();
                Err(format!("no login method `{wanted}`; the agent offers: {}", known.join(", ")))
            }
        };
    }
    match methods {
        [only] => Ok(Choice::Use(only)),
        _ => Ok(Choice::Ask),
    }
}

/// A typed answer: a number from the printed list, or Enter for the first.
pub(super) fn by_number<'a>(methods: &'a [Method], answer: &str) -> Option<&'a Method> {
    let answer = answer.trim();
    if answer.is_empty() {
        return methods.first();
    }
    let number: usize = answer.parse().ok()?;
    methods.get(number.checked_sub(1)?)
}

#[cfg(test)]
#[path = "methods_tests.rs"]
mod methods_tests;
