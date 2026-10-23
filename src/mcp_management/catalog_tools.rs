use super::{operation::Operation, schemas};
use serde_json::json;

pub(super) fn operations() -> Vec<Operation> {
    let mut operations = Vec::new();
    macro_rules! add {
        ($name:literal, $method:literal, $path:literal, $desc:literal) => {
            operations.push(Operation {
                name: $name,
                method: $method,
                path: $path,
                description: $desc,
                body: None,
            });
        };
        ($name:literal, $method:literal, $path:literal, $desc:literal, $body:expr) => {
            operations.push(Operation {
                name: $name,
                method: $method,
                path: $path,
                description: $desc,
                body: Some($body),
            });
        };
    }
    add!("settings_skills_import", "POST", "/skills/import", "Import a skills ZIP archive encoded as base64 (maximum 16 MiB compressed, 32 MiB expanded).", schemas::object(json!({"archiveBase64":{"type":"string","maxLength":22369628}}), &["archiveBase64"]));
    add!(
        "settings_mcp_list",
        "GET",
        "/mcp/servers",
        "List built-in and custom MCP servers without secret values."
    );
    add!(
        "settings_mcp_upsert",
        "PUT",
        "/mcp/servers/{id}",
        "Create or patch MCP server configuration. Secret values can be set but never read.",
        schemas::mcp()
    );
    add!(
        "settings_mcp_remove",
        "DELETE",
        "/mcp/servers/{id}",
        "Remove a custom MCP server."
    );
    add!(
        "settings_mcp_enabled",
        "POST",
        "/mcp/servers/{id}/enabled",
        "Enable or disable an MCP server for new sessions.",
        schemas::enabled()
    );
    add!(
        "settings_mcp_tools",
        "GET",
        "/mcp/servers/{id}/tools",
        "Read a configured server's tool definitions."
    );
    add!(
        "settings_mcp_login_start",
        "POST",
        "/mcp/servers/{id}/login",
        "Start MCP OAuth login and return the URL for the user to open."
    );
    add!(
        "settings_mcp_login_finish",
        "POST",
        "/mcp/servers/{id}/login/finish",
        "Finish MCP OAuth login using the user's returned callback URL.",
        schemas::object(json!({"url":{"type":"string"}}), &["url"])
    );
    add!(
        "settings_mcp_logout",
        "POST",
        "/mcp/servers/{id}/logout",
        "Remove saved MCP OAuth credentials."
    );
    add!(
        "settings_skills_list",
        "GET",
        "/skills",
        "List reusable skills."
    );
    add!(
        "settings_skill_get",
        "GET",
        "/skills/{id}",
        "Read a reusable skill."
    );
    add!(
        "settings_skill_create",
        "POST",
        "/skills",
        "Create a reusable skill from its text.",
        schemas::skill()
    );
    add!(
        "settings_skill_update",
        "PUT",
        "/skills/{id}",
        "Replace a reusable skill's text and metadata.",
        schemas::skill()
    );
    add!(
        "settings_skill_remove",
        "DELETE",
        "/skills/{id}",
        "Delete a reusable skill."
    );
    operations
}
