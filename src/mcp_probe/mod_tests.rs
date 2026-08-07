//! The probe's outcome, as the settings page receives it.

use serde_json::json;

use super::*;
use crate::mcp_registry::builtin::BuiltinFlags;
use crate::mcp_registry::spec::{Arg, Presence, ServerSpec};

fn registry(spec: ServerSpec) -> McpRegistry {
    McpRegistry::from_specs(vec![spec], BuiltinFlags::default())
}

#[tokio::test]
async fn a_server_that_is_not_declared_is_unavailable_rather_than_a_failure() {
    let registry = registry(ServerSpec::stdio("known", "/bin/true", Vec::new(), Vec::new()));

    let outcome = catalog(&registry, "unknown", "/tmp").await;

    assert!(matches!(outcome, Catalog::Unavailable { .. }), "{outcome:?}");
}

/// A presence condition is opman's own decision not to offer the server, so it must not
/// read as the server being broken.
#[tokio::test]
async fn an_unmet_presence_condition_is_unavailable_not_failed() {
    let spec = ServerSpec::stdio("gated", "/bin/true", Vec::new(), Vec::new())
        .with_presence(Presence::Env("OPMAN_PROBE_ABSENT_VAR".into()));
    let registry = registry(spec);

    let outcome = catalog(&registry, "gated", "/tmp").await;

    assert!(matches!(outcome, Catalog::Unavailable { .. }), "{outcome:?}");
}

/// `${session}` is why this exists: without a stand-in id, three built-ins would report
/// themselves unavailable on a page whose entire job is to list their tools.
#[tokio::test]
async fn a_session_bound_server_still_probes() {
    let spec = ServerSpec::stdio(
        "bound",
        "/bin/sh",
        vec![Arg::lit("-c"), Arg::lit("exit 0"), Arg::SessionId],
        Vec::new(),
    );
    let registry = registry(spec);

    let outcome = catalog(&registry, "bound", "/tmp").await;

    // It is launched — it just has nothing to say, which is a `Failed`, not the refusal
    // to launch that a missing session id used to produce.
    assert!(matches!(outcome, Catalog::Failed { .. }), "{outcome:?}");
}

#[test]
fn the_outcome_is_tagged_so_the_page_can_branch_on_it() {
    let listed = Catalog::Listed {
        server: Some(ServerInfo {
            name: "fake".into(),
            version: None,
        }),
        tools: vec![ToolDef {
            name: "echo".into(),
            title: None,
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema: Value::Null,
            annotations: Value::Null,
        }],
    };

    let wire = serde_json::to_value(&listed).expect("the outcome should serialize");

    assert_eq!(wire["status"], "listed");
    assert_eq!(wire["tools"][0]["inputSchema"]["type"], "object");
    // Absent halves of the definition are omitted, not sent as null for the page to test.
    assert!(wire["tools"][0].get("outputSchema").is_none());
    assert!(wire["server"].get("version").is_none());
}
