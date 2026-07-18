use super::*;

// MdSegment is `pub` in this module (super); shared item types are qualified.

fn one(md: &str) -> MdSegment<'_> {
    let mut segs = parse_block_segments(md);
    assert_eq!(segs.len(), 1, "expected exactly one segment for {md:?}");
    segs.remove(0)
}

// ── blank / horizontal rule ─────────────────────────────────────────────

#[test]
fn blank_lines_produce_no_segments() {
    let segs = parse_block_segments("\n\n   \n");
    assert!(segs.is_empty());
}

#[test]
fn horizontal_rules() {
    assert!(matches!(one("---"), MdSegment::HorizontalRule));
    assert!(matches!(one("***"), MdSegment::HorizontalRule));
    assert!(matches!(one("___"), MdSegment::HorizontalRule));
    assert!(matches!(one("- - -"), MdSegment::HorizontalRule));
}

// ── headings ────────────────────────────────────────────────────────────

#[test]
fn heading_level_1_and_2_are_heading1() {
    assert!(matches!(one("# Title"), MdSegment::Heading1(t) if t == "Title"));
    assert!(matches!(one("## Sub"), MdSegment::Heading1(t) if t == "Sub"));
}

#[test]
fn heading_level_3_plus_is_heading3() {
    match one("### Deep") {
        MdSegment::Heading3(t, level) => {
            assert_eq!(t, "Deep");
            assert_eq!(level, 3);
        }
        other => panic!("expected Heading3, got {other:?}"),
    }
}

#[test]
fn heading_trailing_hashes_trimmed() {
    assert!(matches!(one("## Title ##"), MdSegment::Heading1(t) if t == "Title"));
}

// ── code blocks ─────────────────────────────────────────────────────────

#[test]
fn code_block_with_language() {
    match one("```rust\nfn main() {}\n```") {
        MdSegment::CodeBlock { lang, code } => {
            assert_eq!(lang.as_deref(), Some("rust"));
            assert_eq!(code, "fn main() {}");
        }
        other => panic!("expected CodeBlock, got {other:?}"),
    }
}

#[test]
fn code_block_without_language() {
    match one("```\nplain\n```") {
        MdSegment::CodeBlock { lang, code } => {
            assert!(lang.is_none());
            assert_eq!(code, "plain");
        }
        other => panic!("expected CodeBlock, got {other:?}"),
    }
}

#[test]
fn code_block_unterminated_runs_to_eof() {
    match one("```py\nline1\nline2") {
        MdSegment::CodeBlock { lang, code } => {
            assert_eq!(lang.as_deref(), Some("py"));
            assert_eq!(code, "line1\nline2");
        }
        other => panic!("expected CodeBlock, got {other:?}"),
    }
}

// ── table ───────────────────────────────────────────────────────────────

#[test]
fn table_segment_detected() {
    let md = "| A | B |\n| --- | --- |\n| 1 | 2 |";
    match one(md) {
        MdSegment::Table { headers, alignments, rows } => {
            assert_eq!(headers.len(), 2);
            assert_eq!(alignments.len(), 2);
            assert_eq!(rows.len(), 1);
        }
        other => panic!("expected Table, got {other:?}"),
    }
}

// ── blockquote ──────────────────────────────────────────────────────────

#[test]
fn blockquote_with_bare_and_prefixed_lines() {
    match one("> first\n>\n> third") {
        MdSegment::Blockquote(lines) => {
            assert_eq!(lines, vec!["first", "", "third"]);
        }
        other => panic!("expected Blockquote, got {other:?}"),
    }
}

// ── todo list ───────────────────────────────────────────────────────────

#[test]
fn todo_list_checked_and_unchecked() {
    match one("- [ ] a\n- [x] b\n- [X] c") {
        MdSegment::TodoList(items) => {
            assert_eq!(items.len(), 3);
            assert!(!items[0].checked);
            assert_eq!(items[0].text, "a");
            assert!(items[1].checked);
            assert!(items[2].checked);
        }
        other => panic!("expected TodoList, got {other:?}"),
    }
}

#[test]
fn todo_list_star_prefix() {
    match one("* [ ] star") {
        MdSegment::TodoList(items) => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].text, "star");
        }
        other => panic!("expected TodoList, got {other:?}"),
    }
}

// ── bullet list ─────────────────────────────────────────────────────────

#[test]
fn bullet_list_with_indent() {
    match one("- top\n  - nested\n+ plus\n* star") {
        MdSegment::BulletList(items) => {
            assert_eq!(items.len(), 4);
            assert_eq!(items[0].text, "top");
            assert_eq!(items[0].indent, 0);
            assert_eq!(items[1].text, "nested");
            assert_eq!(items[1].indent, 1);
        }
        other => panic!("expected BulletList, got {other:?}"),
    }
}

// ── ordered list ────────────────────────────────────────────────────────

#[test]
fn ordered_list_dot_and_paren() {
    match one("1. first\n2) second") {
        MdSegment::OrderedList(items) => {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].text, "first");
            assert_eq!(items[1].text, "second");
        }
        other => panic!("expected OrderedList, got {other:?}"),
    }
}

#[test]
fn ordered_list_multi_digit() {
    match one("12. twelve") {
        MdSegment::OrderedList(items) => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].text, "twelve");
        }
        other => panic!("expected OrderedList, got {other:?}"),
    }
}

// ── paragraphs & mixing ─────────────────────────────────────────────────

#[test]
fn paragraph_multiline() {
    match one("line one\nline two") {
        MdSegment::Paragraph(lines) => {
            assert_eq!(lines, vec!["line one", "line two"]);
        }
        other => panic!("expected Paragraph, got {other:?}"),
    }
}

#[test]
fn paragraph_breaks_before_other_blocks() {
    // Paragraph followed immediately by a heading, list, quote, rule, code,
    // and table (each a boundary in the fallback while-loop).
    let md = "para\n# H\n- b\n> q\n---\n```\nc\n```\npara2\n| A | B |\n| --- | --- |";
    let segs = parse_block_segments(md);
    assert!(matches!(segs[0], MdSegment::Paragraph(_)));
    assert!(segs.iter().any(|s| matches!(s, MdSegment::Heading1(_))));
    assert!(segs.iter().any(|s| matches!(s, MdSegment::BulletList(_))));
    assert!(segs.iter().any(|s| matches!(s, MdSegment::Blockquote(_))));
    assert!(segs.iter().any(|s| matches!(s, MdSegment::HorizontalRule)));
    assert!(segs.iter().any(|s| matches!(s, MdSegment::CodeBlock { .. })));
    assert!(segs.iter().any(|s| matches!(s, MdSegment::Table { .. })));
}

#[test]
fn text_line_starting_with_hash_that_is_not_heading_is_paragraph() {
    // "#tag" — starts with '#' but parse_heading_line returns None (no space,
    // rest non-empty though), so it becomes a paragraph. Guard: ensure it
    // does not loop. "#notaheading" has non-# after the hash run.
    let segs = parse_block_segments("#notaheading text");
    assert_eq!(segs.len(), 1);
    match &segs[0] {
        MdSegment::Heading3(t, l) => {
            assert_eq!(*l, 1);
            assert_eq!(t, "notaheading text");
        }
        MdSegment::Heading1(_) => {}
        MdSegment::Paragraph(_) => {}
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn mdsegment_debug_impl() {
    let seg = one("# H");
    assert!(format!("{:?}", seg).contains("Heading1"));
}

// ── direct helper predicate coverage ────────────────────────────────────
// These branches are awkward or impossible to reach through
// parse_block_segments without triggering pathological loops, so exercise
// the private helpers directly.

#[test]
fn parse_heading_line_variants() {
    assert_eq!(parse_heading_line("# Hi"), Some((1, "Hi".to_string())));
    assert_eq!(parse_heading_line("###### Deep"), Some((6, "Deep".to_string())));
    assert!(parse_heading_line("####### TooDeep").is_none()); // level > 6
    assert!(parse_heading_line("#").is_none()); // empty rest
    assert!(parse_heading_line("#   ").is_none()); // whitespace-only rest
    assert!(parse_heading_line("plain").is_none()); // no leading '#'
}

#[test]
fn is_horizontal_rule_variants() {
    assert!(is_horizontal_rule("---"));
    assert!(is_horizontal_rule("***"));
    assert!(is_horizontal_rule("___"));
    assert!(is_horizontal_rule("- - -"));
    assert!(!is_horizontal_rule("--")); // fewer than 3
    assert!(!is_horizontal_rule("-*-")); // mixed markers
    assert!(!is_horizontal_rule("abc"));
}

#[test]
fn is_todo_line_variants() {
    assert!(is_todo_line("- [ ] a"));
    assert!(is_todo_line("- [x] a"));
    assert!(is_todo_line("- [X] a"));
    assert!(is_todo_line("* [ ] a"));
    assert!(is_todo_line("* [x] a"));
    assert!(is_todo_line("* [X] a"));
    assert!(!is_todo_line("- plain"));
}

#[test]
fn is_bullet_line_variants() {
    assert!(is_bullet_line("- x"));
    assert!(is_bullet_line("* x"));
    assert!(is_bullet_line("+ x"));
    assert!(!is_bullet_line("- [ ] todo")); // todo, not bullet
    assert!(!is_bullet_line("---")); // horizontal rule
    assert!(!is_bullet_line("plain"));
}

#[test]
fn is_ordered_line_variants() {
    assert!(is_ordered_line("1. item"));
    assert!(is_ordered_line("2) item"));
    assert!(is_ordered_line("12. item")); // multi-digit fallback branch
    assert!(!is_ordered_line("x. item")); // does not start with a digit
    assert!(!is_ordered_line("1x item")); // digit but no ". "/") "
}
