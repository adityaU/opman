//! Tool input schemas mirror the Settings API bodies.
use serde_json::{json, Value};

pub fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn strings() -> Value {
    json!({"type":"array","items":{"type":"string"}})
}
fn string_map() -> Value {
    json!({"type":"object","additionalProperties":{"type":"string"}})
}
pub fn enabled() -> Value {
    object(json!({"enabled":{"type":"boolean"}}), &["enabled"])
}

pub fn appearance() -> Value {
    object(
        json!({"appearance":{"enum":["system","light","dark"]},"themeMode":{"enum":["glassy","flat"]},"density":{"enum":["compact","default","roomy"]}}),
        &[],
    )
}
pub fn server(create: bool) -> Value {
    object(
        json!({"name":{"type":"string"},"url":{"type":"string"},"username":{"type":"string"},"password":{"type":"string","writeOnly":true}}),
        if create { &["name", "url"] } else { &[] },
    )
}
pub fn skill() -> Value {
    object(
        json!({"name":{"type":"string"},"title":{"type":"string"},"description":{"type":"string"},"content":{"type":"string"},"requires":strings()}),
        &["name", "description", "content"],
    )
}
pub fn native_tools() -> Value {
    object(
        json!({"setting":{"oneOf":[{"enum":["all","off"]}, object(json!({"disabled":strings()}), &["disabled"])]}}),
        &["setting"],
    )
}
pub fn agent() -> Value {
    object(
        json!({
            "displayName":{"type":"string"},"command":{"type":"string"},"args":strings(),
            "runner":{"type":"string"},"clientName":{"type":"string"},
            "clientCaps":object(json!({"readTextFile":{"type":"boolean"},"writeTextFile":{"type":"boolean"},"terminal":{"type":"boolean"}}), &[]),
            "injectMcp":{"type":"boolean"},"defaultMode":{"type":"string"},"defaultModel":{"type":"string"},
            "modesAreAgents":{"type":"boolean"},"subagentTranscripts":{"type":"boolean"},"enabled":{"type":"boolean"},
            "envRemove":strings(),"envSet":string_map(),"envUnset":strings()
        }),
        &[],
    )
}
pub fn mcp() -> Value {
    object(
        json!({
            "type":{"enum":["stdio","http","sse"]},"command":{"type":"string"},"args":strings(),
            "url":{"type":"string"},"auth":{"type":"string"},"runners":strings(),
            "timeoutSecs":{"type":["integer","null"],"minimum":1},
            "envSet":string_map(),"envRemove":strings(),"headersSet":string_map(),"headersRemove":strings()
        }),
        &[],
    )
}
pub fn limits() -> Value {
    let model = object(
        json!({"providerID":{"type":"string"},"modelID":{"type":"string","minLength":1}}),
        &["providerID", "modelID"],
    );
    let member = object(
        json!({"runner":{"type":"string"},"model":model,"permission":{"type":"string"}}),
        &["runner", "model"],
    );
    let pair = object(
        json!({"runner":{"type":"string"},"model":{"oneOf":[model,{"type":"null"}]},"permission":{"type":["string","null"]}}),
        &["runner"],
    );
    object(
        json!({
            "policy":{"enum":["ask","auto","off"]},"nearPercent":{"type":"integer","minimum":1,"maximum":99},
            "pairs":{"type":"array","items":pair},
            "modelSets":{"type":"array","items":object(json!({"id":{"type":"string"},"name":{"type":"string"},"members":{"type":"array","minItems":2,"items":member}}), &["id","name","members"])}
        }),
        &["policy", "nearPercent", "pairs", "modelSets"],
    )
}
pub fn keybindings() -> Value {
    let binding = object(
        json!({"key":{"type":"string"},"command":{"type":"string"},"when":{"type":"string"},"mode":{"enum":["normal","vim"]},"platform":{"type":"string"},"target":{"type":"string"},"browser":{"type":"string"},"group":{"type":"string"},"label":{"type":"string"}}),
        &["key", "command"],
    );
    object(
        json!({
            "mode":{"enum":["normal","vim"]},"leader":{"type":"string"},"localLeader":{"type":"string"},"chordTimeoutMs":{"type":"integer"},
            "whichKey":object(json!({"enabled":{"type":"boolean"},"delayMs":{"type":"integer"},"timeoutMs":{"type":"integer"},"sortBy":{"enum":["group","key","label"]}}), &[]),
            "bindings":{"type":"array","items":binding}
        }),
        &[],
    )
}
