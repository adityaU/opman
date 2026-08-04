use super::*;

#[test]
fn markdown_to_blocks_plain_has_no_table_blocks() {
    let result = markdown_to_blocks("# Hello\n\nSome text.");
    assert!(!result.blocks.is_empty());
    assert!(result.table_blocks.is_empty());
    assert!(result
        .blocks
        .iter()
        .all(|b| b["type"] != serde_json::json!("table")));
}

#[test]
fn markdown_to_blocks_separates_table_into_table_blocks() {
    let md = "Intro\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\nOutro";
    let result = markdown_to_blocks(md);
    assert_eq!(result.table_blocks.len(), 1);
    assert_eq!(result.table_blocks[0]["type"], serde_json::json!("table"));
    // The regular blocks must not contain the table.
    assert!(result
        .blocks
        .iter()
        .all(|b| b["type"] != serde_json::json!("table")));
    // Intro and outro should be present as markdown blocks.
    assert!(!result.blocks.is_empty());
}

#[test]
fn markdown_to_blocks_empty_input() {
    let result = markdown_to_blocks("");
    assert!(result.blocks.is_empty());
    assert!(result.table_blocks.is_empty());
}

#[test]
fn markdown_to_blocks_only_table() {
    let md = "| A | B |\n| --- | --- |\n| 1 | 2 |";
    let result = markdown_to_blocks(md);
    assert!(result.blocks.is_empty());
    assert_eq!(result.table_blocks.len(), 1);
}

#[test]
fn shared_type_derives() {
    // Exercise the derived impls on the shared enums/structs.
    let a = TableAlign::Center;
    let b = a; // Copy
    assert_eq!(a, b); // PartialEq
    assert!(format!("{:?}", a).contains("Center")); // Debug

    let li = ListItem {
        text: "x".into(),
        indent: 2,
    };
    assert!(format!("{:?}", li).contains("indent"));

    let td = TodoItem {
        text: "y".into(),
        checked: true,
    };
    assert!(format!("{:?}", td).contains("checked"));

    let style = InlineStyle::BoldItalic; // Copy + Debug
    let _copy = style;
    assert!(format!("{:?}", style).contains("BoldItalic"));
}
