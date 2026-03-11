use ratatui::style::Color;

use super::colors::hex_to_color;
use super::parsing::{resolve_color, strip_jsonc_comments};
use serde_json::Value;

#[test]
fn test_hex_to_color() {
    assert_eq!(hex_to_color("#fab283"), Color::Rgb(0xfa, 0xb2, 0x83));
    assert_eq!(hex_to_color("#000000"), Color::Rgb(0, 0, 0));
    assert_eq!(hex_to_color("#ffffff"), Color::Rgb(255, 255, 255));
    assert_eq!(hex_to_color("fab283"), Color::Rgb(0xfa, 0xb2, 0x83));
}

#[test]
fn test_strip_jsonc_comments() {
    let input = r#"
{
    // This is a comment
    "theme": "opencode", /* inline */
    "foo": "bar"
}
"#;
    let stripped = strip_jsonc_comments(input);
    assert!(!stripped.contains("// This is a comment"));
    assert!(!stripped.contains("/* inline */"));
    assert!(stripped.contains("\"theme\": \"opencode\","));
}

#[test]
fn test_resolve_color_direct_hex() {
    let defs = serde_json::Map::new();
    let value = Value::String("#fab283".to_string());
    assert_eq!(
        resolve_color(&value, &defs, "dark"),
        Some("#fab283".to_string())
    );
}

#[test]
fn test_resolve_color_ref() {
    let mut defs = serde_json::Map::new();
    defs.insert(
        "darkStep9".to_string(),
        Value::String("#fab283".to_string()),
    );
    let value = Value::String("darkStep9".to_string());
    assert_eq!(
        resolve_color(&value, &defs, "dark"),
        Some("#fab283".to_string())
    );
}

#[test]
fn test_resolve_color_dark_light_object() {
    let mut defs = serde_json::Map::new();
    defs.insert(
        "darkStep9".to_string(),
        Value::String("#fab283".to_string()),
    );
    defs.insert(
        "lightStep9".to_string(),
        Value::String("#3b7dd8".to_string()),
    );

    let value = serde_json::json!({
        "dark": "darkStep9",
        "light": "lightStep9"
    });

    assert_eq!(
        resolve_color(&value, &defs, "dark"),
        Some("#fab283".to_string())
    );
    assert_eq!(
        resolve_color(&value, &defs, "light"),
        Some("#3b7dd8".to_string())
    );
}
