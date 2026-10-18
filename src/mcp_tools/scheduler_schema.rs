//! MCP descriptions for the app's persistent routine scheduler.

use super::scheduler::SchedulerTool;
use super::schema::{boolean, choice, object, string, tool};
use serde_json::{json, Value};

pub fn definition(kind: SchedulerTool) -> Value {
    let (description, properties, required) = match kind {
        SchedulerTool::List => (
            "List persistent routines and their recent runs in the current project. Requires the running opman app.",
            vec![("all_projects", boolean("Include routines from every project. Default false."))],
            vec![],
        ),
        SchedulerTool::Create => (
            "Create a persistent routine in opman's Routines scheduler. It survives this tool call and runs while the app is running. Scheduled routines use a five-field cron expression and an IANA timezone (default UTC). New sessions use the current project; existing sessions must belong to it.",
            routine_fields(),
            vec!["name", "prompt"],
        ),
        SchedulerTool::Update => {
            let mut fields = routine_fields();
            fields.push(("routine_id", string("ID of the routine to update.")));
            (
                "Update a routine in the current project. Only supplied fields change. Set enabled=false to pause it. Changes appear in opman's Routines page.",
                fields,
                vec!["routine_id"],
            )
        }
        SchedulerTool::Delete => (
            "Delete a persistent routine from the current project. Future scheduled runs stop.",
            vec![("routine_id", string("ID of the routine to delete."))],
            vec!["routine_id"],
        ),
        SchedulerTool::Run => (
            "Run a routine from the current project now, using the same routine engine as the app. This sends the routine prompt to its target session; it does not wait for the agent's full reply.",
            vec![("routine_id", string("ID of the routine to run now."))],
            vec!["routine_id"],
        ),
    };
    let mut definition = tool(
        kind.name(),
        description,
        object(properties, &required),
        output(kind),
    );
    definition["inputSchema"]["additionalProperties"] = json!(false);
    definition["annotations"] = json!({
        "readOnlyHint": kind == SchedulerTool::List,
        "destructiveHint": matches!(kind, SchedulerTool::Update | SchedulerTool::Delete),
        "idempotentHint": matches!(kind, SchedulerTool::List | SchedulerTool::Update | SchedulerTool::Delete),
        "openWorldHint": kind == SchedulerTool::Run,
    });
    definition
}

fn routine_fields() -> Vec<(&'static str, Value)> {
    vec![
        ("name", string("Routine name.")),
        (
            "prompt",
            string("Message sent to the agent when the routine runs."),
        ),
        (
            "trigger",
            choice(
                &["scheduled", "manual", "on_session_idle", "daily_summary"],
                "Default scheduled; scheduled requires cron_expr.",
            ),
        ),
        (
            "enabled",
            boolean("Whether scheduled execution is enabled. Default true."),
        ),
        (
            "cron_expr",
            string("Five-field cron expression, for example '0 9 * * 1-5'."),
        ),
        (
            "timezone",
            string("IANA timezone such as America/New_York. Default UTC."),
        ),
        (
            "target_mode",
            choice(
                &["new_session", "existing_session"],
                "Default new_session, or existing_session when session_id is provided.",
            ),
        ),
        (
            "session_id",
            string("Target session ID for existing_session mode, in the current project."),
        ),
        (
            "provider_id",
            string("Provider for a new session, if supported by the runner."),
        ),
        (
            "model_id",
            string("Model for a new session, if supported by the runner."),
        ),
    ]
}

fn output(kind: SchedulerTool) -> Value {
    match kind {
        SchedulerTool::List => object(
            vec![
                (
                    "routines",
                    json!({"type":"array", "items":{"type":"object"}}),
                ),
                ("runs", json!({"type":"array", "items":{"type":"object"}})),
            ],
            &["routines", "runs"],
        ),
        SchedulerTool::Delete => object(
            vec![
                ("deleted", boolean("Whether the routine was deleted.")),
                ("routine_id", string("Deleted routine ID.")),
            ],
            &["deleted", "routine_id"],
        ),
        SchedulerTool::Create | SchedulerTool::Update => object(
            vec![
                ("id", string("Routine ID.")),
                ("name", string("Routine name.")),
            ],
            &["id", "name"],
        ),
        SchedulerTool::Run => object(
            vec![
                ("id", string("Run ID.")),
                ("routine_id", string("Routine ID.")),
                ("status", string("Execution status.")),
            ],
            &["id", "routine_id", "status"],
        ),
    }
}
