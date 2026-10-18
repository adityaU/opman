//! What the sandbox needs from the outside world, as a trait so tests can fake it.

use futures::future::BoxFuture;
use serde_json::Value;

/// A file or image a tool returned. Attached to the final result, never inlined.
#[derive(Clone, Debug, PartialEq)]
pub struct Attachment {
    pub mime: String,
    /// Base64, exactly as the tool sent it.
    pub data: String,
}

/// One successful tool result.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolOutput {
    pub value: Value,
    pub attachments: Vec<Attachment>,
}

impl ToolOutput {
    #[cfg(test)]
    pub fn plain(value: Value) -> Self {
        Self {
            value,
            attachments: Vec::new(),
        }
    }
}

pub trait ToolHost: Send + Sync + 'static {
    /// Every namespace with its tool names, in display order.
    fn namespaces(&self) -> Vec<(String, Vec<String>)>;
    /// Run one tool. `Err` is the message the script's `Error` will carry.
    fn call<'a>(
        &'a self,
        namespace: &'a str,
        tool: &'a str,
        input: Value,
    ) -> BoxFuture<'a, Result<ToolOutput, String>>;
    /// `tools.$codemode.search`.
    fn search(&self, query: &str) -> Value;
    /// The message for touching a namespace (`tool` is `None`) or tool that is not there.
    fn unknown(&self, namespace: &str, tool: Option<&str>) -> String;
}
