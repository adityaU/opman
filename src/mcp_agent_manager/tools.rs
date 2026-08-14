//! The tool definitions the runner reads.
//!
//! `model` and `effort` are `required` on both tools that start a turn. The schema is only
//! half the contract — a runner may or may not enforce it — so [`super::dispatch`] refuses
//! the same two arguments again on the way in. What the schema adds is that the agent sees
//! the requirement before it calls, and is told which tool answers it.

use serde_json::{json, Value};

/// Every runner slot a caller may name. Kept as one list so the four occurrences cannot
/// drift apart.
const RUNNERS: [&str; 4] = ["opencode", "claude-code", "claude", "codex"];

fn runner_field(purpose: &str) -> Value {
    json!({ "type": "string", "enum": RUNNERS, "description": purpose })
}

fn delivery_field() -> Value {
    json!({
        "type": "string",
        "enum": ["immediate", "queued"],
        "default": "immediate",
        "description": "immediate steers the target now; queued waits for its next idle turn.",
    })
}

/// The model and effort every dispatched turn must name, plus the optional provider.
fn dispatch_fields() -> Value {
    json!({
        "model": {
            "type": "string",
            "description": "Required. A model id from agent_runner_options — not a display \
                            name. Omitting it would silently inherit whatever the target \
                            last ran, which the caller cannot see.",
        },
        "effort": {
            "type": "string",
            "description": "Required. A reasoning effort the chosen model supports, from \
                            agent_runner_options (commonly low, medium, high).",
        },
        "provider": {
            "type": "string",
            "description": "Provider id for the model, when the runner needs one \
                            disambiguated (OpenCode). Optional.",
        },
        "permission": {
            "type": "string",
            "description": "Permission mode for the turn, named in the target runner's own \
                            vocabulary — claude: default/acceptEdits/plan/dontAsk/\
                            bypassPermissions; codex: read-only/agent/agent-full-access. \
                            agent_runner_options lists them under 'permission_modes'. \
                            Optional: omitted, agent_start inherits the calling session's \
                            mode when the new session lands on the same runner, and \
                            agent_send leaves the target's mode alone.",
        },
    })
}

/// Merge the shared dispatch fields into a tool's own properties.
fn properties(own: Value) -> Value {
    let mut merged = dispatch_fields();
    let (Some(target), Some(own)) = (merged.as_object_mut(), own.as_object()) else {
        return merged;
    };
    for (key, value) in own {
        target.insert(key.clone(), value.clone());
    }
    merged
}

pub(super) fn definitions() -> Value {
    json!([
        {
            "name": "agent_send",
            "description": "Send a message to the parent agent (omit 'to') or any agent id. \
                            Requires model and effort — call agent_runner_options first if \
                            you do not already know what the runner accepts.",
            "inputSchema": {
                "type": "object",
                "properties": properties(json!({
                    "to": { "type": "string", "description": "Target agent id, or 'parent'. Omit to address the parent." },
                    "message": { "type": "string", "description": "The message text." },
                    "delivery": delivery_field(),
                    "runner": runner_field("Switch the target to this runner before sending. Optional."),
                })),
                "required": ["message", "model", "effort"],
            },
        },
        {
            "name": "agent_start",
            "description": "Start a new agent session on any available runner. Requires \
                            model and effort — call agent_runner_options first if you do \
                            not already know what the runner accepts. Pass 'permission' to \
                            decide what the new session may do without asking (e.g. \
                            bypassPermissions, agent-full-access); it is applied to the \
                            session before its first turn. When a caller agent starts a \
                            session with a message, the opening prompt includes that \
                            caller's id and instructions to report the final work back \
                            with agent_send.",
            "inputSchema": {
                "type": "object",
                "properties": properties(json!({
                    "message": { "type": "string", "description": "Opening message. Omit to create the session without starting a turn." },
                    "title": { "type": "string", "description": "Session title. Defaults to 'Agent session'." },
                    "delivery": delivery_field(),
                    "runner": runner_field("Runner to start on. Defaults to the caller's own."),
                })),
                "required": ["model", "effort"],
            },
        },
        {
            "name": "agent_progress",
            "description": "Inspect another agent's current busy/idle state, recent \
                            transcript messages, runner, and queued message count.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent_id": { "type": "string", "description": "Agent id, or 'parent'. Omit to ask about the parent." },
                },
            },
        },
        {
            "name": "agent_runner_options",
            "description": "List the models, the reasoning efforts each one supports, and \
                            the runner's permission modes. Call this before agent_send or \
                            agent_start: both require a model and an effort, and \
                            'permission_modes' names what may be passed as 'permission'. \
                            Only connected providers are listed; use 'filter' to reach a \
                            model behind one that is not.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "runner": runner_field("Runner to describe. Omit for the default runner."),
                    "filter": {
                        "type": "string",
                        "description": "Substring matched against each model id and \
                                        provider id, e.g. 'haiku' or 'luna'. Searches every \
                                        provider, not just the connected ones.",
                    },
                },
            },
        },
        {
            "name": "agent_list",
            "description": "List the agent sessions in this project — id, title, runner, \
                            whether each is busy, and how many messages are queued for it. \
                            Use this to address an agent you did not start yourself. \
                            Returns the 20 most recently active by default, plus 'count', \
                            the project total; raise 'offset' to reach older ones.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "offset": {
                        "type": "integer",
                        "description": "How far into the newest-first list to start. \
                                        Defaults to 0.",
                    },
                    "limit": {
                        "type": "integer",
                        "description": "How many agents to return. Defaults to 20, capped \
                                        at 100.",
                    },
                },
            },
        },
        {
            "name": "agent_wait",
            "description": "Block until an agent finishes its current turn, then return \
                            what it said. This is how you get a reply: agent_send and \
                            agent_start hand the message over and return before the target \
                            has answered.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent_id": { "type": "string", "description": "Agent id, or 'parent'. Omit to wait on the parent." },
                    "timeout": {
                        "type": "integer",
                        "description": "Seconds to wait. Defaults to 300, capped at 1800. \
                                        Running out is reported as timed_out, not an error.",
                    },
                },
            },
        },
        {
            "name": "agent_abort",
            "description": "Stop the turn an agent is currently running. The session and \
                            its transcript survive; only the turn in flight is cancelled.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent_id": { "type": "string", "description": "Agent id, or 'parent'. Omit to abort the parent." },
                },
            },
        },
    ])
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tools_tests;
