use serde_json::{json, Value};

pub struct Operation {
    pub name: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub description: &'static str,
    pub body: Option<Value>,
}

impl Operation {
    pub fn definition(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        if self.path.contains("{id}") {
            properties.insert("id".into(), json!({"type":"string", "minLength":1, "description":"Exact id/name from the matching list tool."}));
            required.push("id");
        }
        if let Some(schema) = &self.body {
            properties.insert("body".into(), schema.clone());
            required.push("body");
        }
        json!({"name":self.name,"description":self.description,
            "inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},
            "annotations":self.annotations()})
    }

    fn annotations(&self) -> Value {
        let read_only =
            self.method.eq_ignore_ascii_case("GET") || self.name == "settings_directories_browse";
        let destructive = self.method.eq_ignore_ascii_case("DELETE")
            || matches!(
                self.name,
                "settings_mcp_logout" | "settings_project_remove" | "settings_agents_reset"
            );
        let idempotent = read_only || self.method.eq_ignore_ascii_case("PUT");
        let open_world = self.path.starts_with("/remote/")
            || matches!(
                self.name,
                "settings_mcp_login_start" | "settings_mcp_login_finish"
            );
        json!({
            "readOnlyHint": read_only,
            "destructiveHint": destructive,
            "idempotentHint": idempotent,
            "openWorldHint": open_world,
        })
    }

    pub fn target(&self, args: &Value) -> Result<String, String> {
        if !self.path.contains("{id}") {
            return Ok(self.path.into());
        }
        let id = args
            .get("id")
            .and_then(Value::as_str)
            .ok_or("id is required")?;
        if id.is_empty()
            || matches!(id, "." | "..")
            || id.chars().any(|c| c.is_control() || "/\\?#%".contains(c))
        {
            return Err("Invalid settings id".into());
        }
        let encoded: String = url::form_urlencoded::byte_serialize(id.as_bytes()).collect();
        Ok(self.path.replace("{id}", &encoded))
    }
}

#[cfg(test)]
mod tests {
    use super::Operation;
    use serde_json::json;

    fn operation(method: &'static str) -> Operation {
        Operation {
            name: "test_operation",
            method,
            path: "/settings",
            description: "Test operation.",
            body: None,
        }
    }

    #[test]
    fn definitions_classify_reads_and_mutations_for_clients() {
        let read = operation("GET").definition();
        assert_eq!(
            read["annotations"],
            json!({
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false,
            })
        );

        let update = operation("PUT").definition();
        assert_eq!(update["annotations"]["readOnlyHint"], json!(false));
        assert_eq!(update["annotations"]["idempotentHint"], json!(true));

        let remove = operation("DELETE").definition();
        assert_eq!(remove["annotations"]["readOnlyHint"], json!(false));
        assert_eq!(remove["annotations"]["destructiveHint"], json!(true));
    }

    #[test]
    fn management_list_and_remote_tools_get_specific_hints() {
        let operations = super::super::catalog::operations();
        let find = |name| {
            operations
                .iter()
                .find(|operation| operation.name == name)
                .map(Operation::definition)
        };

        let list = find("settings_mcp_list");
        assert_eq!(
            list.as_ref()
                .map(|definition| &definition["annotations"]["readOnlyHint"]),
            Some(&json!(true))
        );

        let browse = find("settings_directories_browse");
        assert_eq!(
            browse
                .as_ref()
                .map(|definition| &definition["annotations"]["readOnlyHint"]),
            Some(&json!(true))
        );

        let remote = find("settings_server_theme_set");
        assert_eq!(
            remote
                .as_ref()
                .map(|definition| &definition["annotations"]["openWorldHint"]),
            Some(&json!(true))
        );
    }
}
