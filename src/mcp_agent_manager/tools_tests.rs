//! The published contract. These assertions are the reason an agent knows it must pass a
//! model and an effort before it calls.

use super::*;

fn tool(name: &str) -> Value {
    definitions()
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == name).cloned())
        .unwrap_or_else(|| panic!("{name} should be defined"))
}

fn required(name: &str) -> Vec<String> {
    tool(name)["inputSchema"]["required"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn sending_requires_a_message_a_model_and_an_effort() {
    let required = required("agent_send");

    assert!(required.contains(&"message".to_string()), "{required:?}");
    assert!(required.contains(&"model".to_string()), "{required:?}");
    assert!(required.contains(&"effort".to_string()), "{required:?}");
}

/// A message is optional when starting — a session with no opening turn is a legitimate
/// thing to ask for — but the model it would run under is not.
#[test]
fn starting_requires_a_model_and_an_effort_but_not_a_message() {
    let required = required("agent_start");

    assert_eq!(required, vec!["model".to_string(), "effort".to_string()]);
}

#[test]
fn both_dispatching_tools_describe_the_two_required_fields() {
    for name in ["agent_send", "agent_start"] {
        let properties = &tool(name)["inputSchema"]["properties"];
        for field in ["model", "effort"] {
            let description = properties[field]["description"]
                .as_str()
                .unwrap_or_default();
            assert!(
                description.contains("Required"),
                "{name}.{field}: {description}"
            );
            assert!(
                description.contains("agent_runner_options"),
                "{name}.{field} should point at the tool that answers it: {description}"
            );
        }
    }
}

/// The two read-only tools take no model: requiring one there would be a parameter with
/// nothing to apply to.
#[test]
fn the_read_only_tools_require_nothing() {
    for name in ["agent_progress", "agent_runner_options"] {
        assert!(required(name).is_empty(), "{name}");
        let properties = &tool(name)["inputSchema"]["properties"];
        assert!(properties.get("model").is_none(), "{name}");
        assert!(properties.get("effort").is_none(), "{name}");
    }
}

#[test]
fn a_dispatching_tool_keeps_its_own_fields_alongside_the_shared_ones() {
    let properties = &tool("agent_send")["inputSchema"]["properties"];

    for field in [
        "to", "message", "delivery", "runner", "model", "effort", "provider",
    ] {
        assert!(properties.get(field).is_some(), "missing {field}");
    }
}

/// The seven an agent can call. A tool the manager implements but never advertises is a
/// capability no agent will ever discover.
#[test]
fn every_implemented_tool_is_advertised() {
    let defined = definitions();
    let names: Vec<&str> = defined
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| tool["name"].as_str())
                .collect()
        })
        .unwrap_or_default();

    assert_eq!(names.len(), 7, "{names:?}");
    for expected in [
        "agent_send",
        "agent_start",
        "agent_progress",
        "agent_runner_options",
        "agent_list",
        "agent_wait",
        "agent_abort",
    ] {
        assert!(names.contains(&expected), "missing {expected}: {names:?}");
    }
}

/// Neither is a turn, so neither may demand the model and effort a turn needs. Requiring
/// them would be a parameter with nothing to apply to — and would put a wedged agent out
/// of reach of the tool that stops it.
#[test]
fn waiting_and_aborting_require_nothing_and_take_no_model() {
    for name in ["agent_wait", "agent_abort", "agent_list"] {
        assert!(required(name).is_empty(), "{name}");
        let properties = &tool(name)["inputSchema"]["properties"];
        assert!(properties.get("model").is_none(), "{name}");
        assert!(properties.get("effort").is_none(), "{name}");
    }
}

/// The point of `agent_wait` is that the dispatching tools do not carry a reply, and an
/// agent that does not know that will sit polling `agent_progress` instead.
#[test]
fn waiting_explains_that_send_and_start_do_not_return_a_reply() {
    let waiting = tool("agent_wait");
    let description = waiting["description"].as_str().unwrap_or_default();

    assert!(description.contains("agent_send"), "{description}");
    assert!(description.contains("agent_start"), "{description}");
}

/// A capped listing is only safe if the caller is told how to reach what was cut.
#[test]
fn runner_options_documents_the_filter_that_reaches_an_unlisted_model() {
    let options = tool("agent_runner_options");

    assert!(options["inputSchema"]["properties"]["filter"].is_object());
    let description = options["description"].as_str().unwrap_or_default();
    assert!(description.contains("filter"), "{description}");
}

#[test]
fn every_tool_names_the_same_four_runners() {
    for name in ["agent_send", "agent_start", "agent_runner_options"] {
        let listed = tool(name)["inputSchema"]["properties"]["runner"]["enum"].clone();
        assert_eq!(listed, json!(RUNNERS), "{name}");
    }
}
