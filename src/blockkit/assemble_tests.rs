use super::*;

use crate::blockkit as bk;

fn md(seg: bk::MdSegment<'_>) -> String {
    let mut buf = String::new();
    segment_to_markdown(&seg, &mut buf);
    buf
}

// ── segment_to_markdown: every arm ──────────────────────────────────────

#[test]
fn reconstruct_heading1() {
    assert_eq!(md(bk::MdSegment::Heading1("Title".into())), "# Title");
}

#[test]
fn reconstruct_heading3_uses_level() {
    assert_eq!(md(bk::MdSegment::Heading3("Sub".into(), 4)), "#### Sub");
}

#[test]
fn reconstruct_paragraph_multiline() {
    assert_eq!(md(bk::MdSegment::Paragraph(vec!["a", "b"])), "a\nb");
}

#[test]
fn reconstruct_codeblock_with_lang() {
    let seg = bk::MdSegment::CodeBlock {
        lang: Some("rs".into()),
        code: "x".into(),
    };
    assert_eq!(md(seg), "```rs\nx\n```");
}

#[test]
fn reconstruct_codeblock_without_lang() {
    let seg = bk::MdSegment::CodeBlock {
        lang: None,
        code: "x".into(),
    };
    assert_eq!(md(seg), "```\nx\n```");
}

#[test]
fn reconstruct_blockquote() {
    assert_eq!(md(bk::MdSegment::Blockquote(vec!["a", "b"])), "> a\n> b");
}

#[test]
fn reconstruct_bullet_list_with_indent() {
    let seg = bk::MdSegment::BulletList(vec![
        bk::ListItem {
            text: "top".into(),
            indent: 0,
        },
        bk::ListItem {
            text: "child".into(),
            indent: 1,
        },
    ]);
    assert_eq!(md(seg), "- top\n  - child");
}

#[test]
fn reconstruct_ordered_list_numbers() {
    let seg = bk::MdSegment::OrderedList(vec![
        bk::ListItem {
            text: "one".into(),
            indent: 0,
        },
        bk::ListItem {
            text: "two".into(),
            indent: 0,
        },
    ]);
    assert_eq!(md(seg), "1. one\n2. two");
}

#[test]
fn reconstruct_todo_list() {
    let seg = bk::MdSegment::TodoList(vec![
        bk::TodoItem {
            text: "done".into(),
            checked: true,
        },
        bk::TodoItem {
            text: "todo".into(),
            checked: false,
        },
    ]);
    assert_eq!(md(seg), "- [x] done\n- [ ] todo");
}

#[test]
fn reconstruct_horizontal_rule() {
    assert_eq!(md(bk::MdSegment::HorizontalRule), "---");
}

#[test]
fn reconstruct_table_arm_is_noop() {
    let seg = bk::MdSegment::Table {
        headers: vec!["A".into()],
        alignments: vec![bk::TableAlign::Left],
        rows: vec![vec!["1".into()]],
    };
    assert_eq!(md(seg), "");
}

// ── flush_markdown ──────────────────────────────────────────────────────

#[test]
fn flush_markdown_whitespace_only_produces_nothing() {
    let mut buf = String::from("   \n  ");
    let mut blocks: Vec<serde_json::Value> = Vec::new();
    flush_markdown(&mut buf, &mut blocks);
    assert!(blocks.is_empty());
    assert!(buf.is_empty());
}

#[test]
fn flush_markdown_emits_trimmed_block() {
    let mut buf = String::from("  hello  ");
    let mut blocks: Vec<serde_json::Value> = Vec::new();
    flush_markdown(&mut buf, &mut blocks);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0]["type"], serde_json::json!("markdown"));
    assert_eq!(blocks[0]["text"], serde_json::json!("hello"));
    assert!(buf.is_empty());
}

// ── build_table_block ───────────────────────────────────────────────────

#[test]
fn build_table_block_basic() {
    let headers = vec!["A".to_string(), "B".to_string()];
    let aligns = vec![bk::TableAlign::Left, bk::TableAlign::Right];
    let rows = vec![vec!["1".to_string(), "2".to_string()]];
    let block = build_table_block(&headers, &aligns, &rows);
    assert_eq!(block["type"], serde_json::json!("table"));
    let all_rows = block["rows"].as_array().unwrap();
    assert_eq!(all_rows.len(), 2); // header + 1 data row
    assert_eq!(all_rows[0][0]["type"], serde_json::json!("raw_text"));
    assert_eq!(all_rows[0][0]["text"], serde_json::json!("A"));
    let cs = block["column_settings"].as_array().unwrap();
    assert_eq!(cs.len(), 2);
    assert_eq!(cs[0]["align"], serde_json::json!("left"));
    assert_eq!(cs[1]["align"], serde_json::json!("right"));
    assert_eq!(cs[0]["is_wrapped"], serde_json::json!(true));
}

#[test]
fn build_table_block_center_alignment() {
    let block = build_table_block(&["H".to_string()], &[bk::TableAlign::Center], &[]);
    assert_eq!(
        block["column_settings"][0]["align"],
        serde_json::json!("center")
    );
}

#[test]
fn build_table_block_no_alignments_omits_column_settings() {
    let block = build_table_block(&["H".to_string()], &[], &[]);
    assert!(block.get("column_settings").is_none());
}

#[test]
fn build_table_block_truncates_columns_and_rows() {
    let headers: Vec<String> = (0..25).map(|n| n.to_string()).collect();
    let aligns: Vec<bk::TableAlign> = (0..25).map(|_| bk::TableAlign::Left).collect();
    let rows: Vec<Vec<String>> = (0..105)
        .map(|_| (0..25).map(|n| n.to_string()).collect())
        .collect();
    let block = build_table_block(&headers, &aligns, &rows);
    let all_rows = block["rows"].as_array().unwrap();
    assert_eq!(all_rows.len(), 100); // 1 header + 99 data rows max
    assert_eq!(all_rows[0].as_array().unwrap().len(), 20); // max 20 columns
    assert_eq!(block["column_settings"].as_array().unwrap().len(), 20);
}

// ── assemble_blocks (integration of the above) ──────────────────────────

#[test]
fn assemble_blocks_empty() {
    let blocks = assemble_blocks(&[]);
    assert!(blocks.is_empty());
}

#[test]
fn assemble_blocks_joins_segments_with_blank_line() {
    let segs = vec![
        bk::MdSegment::Heading1("Title".into()),
        bk::MdSegment::Paragraph(vec!["body"]),
    ];
    let blocks = assemble_blocks(&segs);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0]["type"], serde_json::json!("markdown"));
    assert_eq!(blocks[0]["text"], serde_json::json!("# Title\n\nbody"));
}

#[test]
fn assemble_blocks_flushes_around_table() {
    let segs = vec![
        bk::MdSegment::Paragraph(vec!["before"]),
        bk::MdSegment::Table {
            headers: vec!["A".into()],
            alignments: vec![bk::TableAlign::Left],
            rows: vec![vec!["1".into()]],
        },
        bk::MdSegment::Paragraph(vec!["after"]),
    ];
    let blocks = assemble_blocks(&segs);
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0]["type"], serde_json::json!("markdown"));
    assert_eq!(blocks[0]["text"], serde_json::json!("before"));
    assert_eq!(blocks[1]["type"], serde_json::json!("table"));
    assert_eq!(blocks[2]["type"], serde_json::json!("markdown"));
    assert_eq!(blocks[2]["text"], serde_json::json!("after"));
}
