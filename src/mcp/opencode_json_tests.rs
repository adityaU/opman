use super::*;
use tempfile::TempDir;

fn read_json(dir: &TempDir) -> serde_json::Value {
    let content = std::fs::read_to_string(dir.path().join("opencode.json")).unwrap();
    serde_json::from_str(&content).unwrap()
}

#[test]
fn writes_all_mcp_servers_when_enabled() {
    let dir = TempDir::new().unwrap();
    write_opencode_json(dir.path(), true, true, true, true).unwrap();
    let v = read_json(&dir);
    let mcp = &v["mcp"];
    assert!(mcp["terminal"].is_object());
    assert!(mcp["neovim"].is_object());
    assert!(mcp["time"].is_object());
    assert!(mcp["ui"].is_object());
    assert_eq!(mcp["terminal"]["type"], "local");
    // terminal disables bash; neovim disables edit.
    assert_eq!(v["permission"]["bash"], "deny");
    assert_eq!(v["permission"]["edit"], "deny");
}

#[test]
fn removes_disabled_servers() {
    let dir = TempDir::new().unwrap();
    // First enable everything.
    write_opencode_json(dir.path(), true, true, true, true).unwrap();
    // Then disable everything; keys should be removed.
    write_opencode_json(dir.path(), false, false, false, false).unwrap();
    let v = read_json(&dir);
    let mcp = &v["mcp"];
    assert!(mcp.get("terminal").is_none());
    assert!(mcp.get("neovim").is_none());
    assert!(mcp.get("time").is_none());
    assert!(mcp.get("ui").is_none());
}

#[test]
fn preserves_existing_unrelated_keys() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("opencode.json"),
        r#"{"model":"my-model","mcp":{"custom":{"type":"local"}}}"#,
    )
    .unwrap();
    write_opencode_json(dir.path(), true, false, false, false).unwrap();
    let v = read_json(&dir);
    assert_eq!(v["model"], "my-model");
    // Pre-existing custom mcp entry survives.
    assert!(v["mcp"]["custom"].is_object());
    // terminal added, neovim not.
    assert!(v["mcp"]["terminal"].is_object());
    assert!(v["mcp"].get("neovim").is_none());
}

#[test]
fn recovers_from_invalid_existing_json() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("opencode.json"), "this is not json{{{").unwrap();
    // Should not error — falls back to an empty object.
    write_opencode_json(dir.path(), true, false, false, false).unwrap();
    let v = read_json(&dir);
    assert!(v["mcp"]["terminal"].is_object());
}

#[test]
fn terminal_only_does_not_deny_edit() {
    let dir = TempDir::new().unwrap();
    write_opencode_json(dir.path(), true, false, false, false).unwrap();
    let v = read_json(&dir);
    assert_eq!(v["permission"]["bash"], "deny");
    assert!(v["permission"].get("edit").is_none());
}

#[test]
fn neovim_only_denies_edit_not_bash() {
    let dir = TempDir::new().unwrap();
    write_opencode_json(dir.path(), false, true, false, false).unwrap();
    let v = read_json(&dir);
    // No terminal → no permission.bash; but neovim → permission.edit deny.
    assert_eq!(v["permission"]["edit"], "deny");
    assert!(v["permission"].get("bash").is_none());
}

#[test]
fn no_permissions_block_when_neither_terminal_nor_neovim() {
    let dir = TempDir::new().unwrap();
    write_opencode_json(dir.path(), false, false, true, true).unwrap();
    let v = read_json(&dir);
    assert!(v.get("permission").is_none());
    assert!(v["mcp"]["time"].is_object());
    assert!(v["mcp"]["ui"].is_object());
}
