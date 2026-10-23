//! A closed list of settings actions. Callers never supply URLs or HTTP methods.
use serde_json::json;

use super::schemas;

pub use super::operation::Operation;

pub fn operations() -> Vec<Operation> {
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
    add!(
        "settings_runner_models",
        "GET",
        "/providers?runner={id}",
        "Fetch a runner's real provider/model ids and capabilities before creating a model set."
    );
    add!(
        "settings_directories_browse",
        "POST",
        "/dirs/browse",
        "Browse folders while adding a project.",
        schemas::object(json!({"path":{"type":"string"}}), &[])
    );
    add!(
        "settings_theme_get",
        "GET",
        "/theme",
        "Read the active theme palette."
    );
    add!(
        "settings_themes_list",
        "GET",
        "/themes",
        "List all theme palettes."
    );
    add!(
        "settings_theme_set",
        "POST",
        "/theme/switch",
        "Set the theme palette by name on all clients.",
        schemas::object(json!({"name":{"type":"string"}}), &["name"])
    );
    add!(
        "settings_appearance_get",
        "GET",
        "/preferences",
        "Read shared appearance, surfaces and density. Null means no shared override."
    );
    add!(
        "settings_appearance_set",
        "PUT",
        "/preferences",
        "Set shared light/dark, glassy/flat and density preferences.",
        schemas::appearance()
    );
    add!(
        "settings_keybindings_get",
        "GET",
        "/keybindings",
        "Read shortcuts and keymap configuration."
    );
    add!(
        "settings_keybindings_set",
        "PUT",
        "/keybindings",
        "Replace keybindings configuration. Read the current configuration first.",
        schemas::keybindings()
    );
    add!(
        "settings_servers_list",
        "GET",
        "/servers",
        "List remote servers. Credentials are never returned."
    );
    add!(
        "settings_server_add",
        "POST",
        "/servers",
        "Add a remote opman server. Credentials are write only.",
        schemas::server(true)
    );
    add!(
        "settings_server_update",
        "PATCH",
        "/servers/{id}",
        "Update a remote server, including its saved credentials.",
        schemas::server(false)
    );
    add!(
        "settings_server_remove",
        "DELETE",
        "/servers/{id}",
        "Remove a remote server from opman."
    );
    add!(
        "settings_server_theme_get",
        "GET",
        "/remote/{id}/api/theme",
        "Read a configured remote server's active theme. Use settings_theme_get for home."
    );
    add!(
        "settings_server_themes_list",
        "GET",
        "/remote/{id}/api/themes",
        "List theme palettes available on a configured remote server."
    );
    add!(
        "settings_server_theme_set",
        "POST",
        "/remote/{id}/api/theme/switch",
        "Set a configured remote server's theme. Use settings_theme_set for home.",
        schemas::object(json!({"name":{"type":"string"}}), &["name"])
    );
    add!(
        "settings_projects_list",
        "GET",
        "/state",
        "List configured projects and current app state."
    );
    add!(
        "settings_project_add",
        "POST",
        "/project/add",
        "Add an existing project folder.",
        schemas::object(
            json!({"path":{"type":"string"},"name":{"type":"string"}}),
            &["path"]
        )
    );
    add!(
        "settings_project_remove",
        "POST",
        "/project/remove",
        "Remove a project from opman without deleting its files.",
        schemas::object(json!({"index":{"type":"integer","minimum":0}}), &["index"])
    );
    add!(
        "settings_project_create",
        "POST",
        "/dirs/create",
        "Create a project directory.",
        schemas::object(
            json!({"parent":{"type":"string"},"name":{"type":"string"}}),
            &["parent", "name"]
        )
    );
    add!(
        "settings_runners_get",
        "GET",
        "/runners/settings",
        "List built-in runner settings and the default runner."
    );
    add!(
        "settings_runner_enabled",
        "PUT",
        "/runners/settings/{id}",
        "Enable or disable a built-in runner.",
        schemas::enabled()
    );
    add!(
        "settings_runner_default",
        "PUT",
        "/runners/default",
        "Set the default runner for new sessions.",
        schemas::object(json!({"runner":{"type":"string"}}), &["runner"])
    );
    add!(
        "settings_agents_list",
        "GET",
        "/acp/agents",
        "List ACP agents, models, supported native tools and configuration."
    );
    add!(
        "settings_agent_upsert",
        "PUT",
        "/acp/agents/{id}",
        "Create or patch an ACP agent. Environment secrets are write only.",
        schemas::agent()
    );
    add!(
        "settings_agent_remove",
        "DELETE",
        "/acp/agents/{id}",
        "Remove a custom ACP agent or restore a built-in agent's defaults."
    );
    add!(
        "settings_agents_reset",
        "DELETE",
        "/acp/agents",
        "Reset all ACP agents to their built-in defaults."
    );
    add!(
        "settings_agent_enabled",
        "POST",
        "/acp/agents/{id}/enabled",
        "Enable or disable an ACP agent.",
        schemas::enabled()
    );
    add!(
        "settings_agent_auth",
        "GET",
        "/acp/agents/{id}/auth",
        "Check agent login status and available login methods."
    );
    add!(
        "settings_agent_native_tools",
        "POST",
        "/acp/agents/{id}/native-tools",
        "Set native tools to all, off, or a list of disabled tool ids.",
        schemas::native_tools()
    );
    add!(
        "settings_runner_limits_get",
        "GET",
        "/runner-limits",
        "Read usage-limit policy, fallback pairs and model sets."
    );
    add!("settings_runner_limits_set", "PUT", "/runner-limits", "Replace usage-limit settings including equivalent model sets. Read the current settings first.", schemas::limits());
    operations.extend(super::catalog_tools::operations());
    operations
}
