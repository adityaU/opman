use super::*;
use ratatui::style::Color;
use serde_json::{json, Value};

#[test]
fn parse_theme_full_with_refs_and_variants() {
    let j = json!({
        "defs": { "d9": "#112233", "l9": "#445566" },
        "theme": {
            "primary": { "dark": "d9", "light": "l9" },
            "secondary": "#abcdef",
            "accent": "d9"
            // remaining fields absent -> fallbacks
        }
    });
    let dark = parse_theme(&j, "dark").unwrap();
    assert_eq!(dark.primary, Color::Rgb(0x11, 0x22, 0x33));
    assert_eq!(dark.secondary, Color::Rgb(0xab, 0xcd, 0xef));
    assert_eq!(dark.accent, Color::Rgb(0x11, 0x22, 0x33));
    // fallback values
    assert_eq!(dark.background, Color::Rgb(0x0a, 0x0a, 0x0a));
    assert_eq!(dark.error, Color::Rgb(0xe0, 0x6c, 0x75));

    let light = parse_theme(&j, "light").unwrap();
    assert_eq!(light.primary, Color::Rgb(0x44, 0x55, 0x66));
}

#[test]
fn parse_theme_missing_defs_errors() {
    let j = json!({ "theme": {} });
    let err = parse_theme(&j, "dark").unwrap_err();
    assert!(err.to_string().contains("defs"));
}

#[test]
fn parse_theme_missing_theme_errors() {
    let j = json!({ "defs": {} });
    let err = parse_theme(&j, "dark").unwrap_err();
    assert!(err.to_string().contains("theme"));
}

#[test]
fn parse_theme_defs_not_object_errors() {
    let j = json!({ "defs": "nope", "theme": {} });
    assert!(parse_theme(&j, "dark").is_err());
}

#[test]
fn resolve_color_direct_hex() {
    let defs = serde_json::Map::new();
    assert_eq!(
        resolve_color(&Value::String("#abc123".into()), &defs, "dark"),
        Some("#abc123".to_string())
    );
}

#[test]
fn resolve_color_ref_found_and_missing() {
    let mut defs = serde_json::Map::new();
    defs.insert("ref1".into(), Value::String("#010203".into()));
    assert_eq!(
        resolve_color(&Value::String("ref1".into()), &defs, "dark"),
        Some("#010203".to_string())
    );
    // Missing ref -> None
    assert_eq!(
        resolve_color(&Value::String("missing".into()), &defs, "dark"),
        None
    );
    // Ref that points to a non-string def -> None
    defs.insert("num".into(), json!(5));
    assert_eq!(
        resolve_color(&Value::String("num".into()), &defs, "dark"),
        None
    );
}

#[test]
fn resolve_color_object_mode_and_dark_fallback() {
    let mut defs = serde_json::Map::new();
    defs.insert("d".into(), Value::String("#111111".into()));
    defs.insert("l".into(), Value::String("#222222".into()));

    let v = json!({ "dark": "d", "light": "l" });
    assert_eq!(
        resolve_color(&v, &defs, "dark"),
        Some("#111111".to_string())
    );
    assert_eq!(
        resolve_color(&v, &defs, "light"),
        Some("#222222".to_string())
    );

    // mode not present -> falls back to "dark"
    let v2 = json!({ "dark": "d" });
    assert_eq!(
        resolve_color(&v2, &defs, "light"),
        Some("#111111".to_string())
    );

    // neither mode nor dark -> None
    let v3 = json!({ "light": "l" });
    assert_eq!(resolve_color(&v3, &defs, "dark"), None);
}

#[test]
fn resolve_color_non_string_non_object_is_none() {
    let defs = serde_json::Map::new();
    assert_eq!(resolve_color(&json!(42), &defs, "dark"), None);
    assert_eq!(resolve_color(&json!(true), &defs, "dark"), None);
    assert_eq!(resolve_color(&Value::Null, &defs, "dark"), None);
    assert_eq!(resolve_color(&json!([1, 2]), &defs, "dark"), None);
}

#[test]
fn resolve_color_nested_object_variant() {
    // A variant whose value is itself an object should resolve recursively.
    let mut defs = serde_json::Map::new();
    defs.insert("deep".into(), Value::String("#0f0f0f".into()));
    let v = json!({ "dark": { "dark": "deep" } });
    assert_eq!(
        resolve_color(&v, &defs, "dark"),
        Some("#0f0f0f".to_string())
    );
}

#[test]
fn strip_jsonc_line_and_block_comments() {
    let input = "{\n  // line comment\n  \"a\": 1, /* block */\n  \"b\": 2\n}";
    let out = strip_jsonc_comments(input);
    assert!(!out.contains("line comment"));
    assert!(!out.contains("block"));
    assert!(out.contains("\"a\": 1,"));
    assert!(out.contains("\"b\": 2"));
    // Still valid JSON after stripping
    let parsed: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["a"], json!(1));
}

#[test]
fn strip_jsonc_preserves_comment_like_text_in_strings() {
    let input = r#"{"url": "http://x.com", "note": "a /* not */ comment"}"#;
    let out = strip_jsonc_comments(input);
    let parsed: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["url"], json!("http://x.com"));
    assert_eq!(parsed["note"], json!("a /* not */ comment"));
}

#[test]
fn strip_jsonc_handles_escaped_quotes_in_strings() {
    let input = r#"{"s": "he said \"// hi\" ok"}"#;
    let out = strip_jsonc_comments(input);
    let parsed: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["s"], json!("he said \"// hi\" ok"));
}

#[test]
fn strip_jsonc_lone_slash_is_kept() {
    // A single '/' not followed by '/' or '*' must be preserved.
    let out = strip_jsonc_comments("a/b");
    assert_eq!(out, "a/b");
}

#[test]
fn strip_jsonc_block_comment_with_newlines_keeps_them() {
    let input = "1/* line1\nline2 */2";
    let out = strip_jsonc_comments(input);
    assert_eq!(out, "1\n2");
}

#[test]
fn strip_jsonc_trailing_backslash_at_string_end() {
    // Escape char at the very end of input inside a string: no following char.
    let input = "\"abc\\";
    let out = strip_jsonc_comments(input);
    assert_eq!(out, "\"abc\\");
}
