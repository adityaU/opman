use super::*;

use crate::blockkit::InlineStyle;

fn as_str<'a>(v: &'a serde_json::Value, key: &str) -> &'a str {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("")
}

// ── parse_inline_elements: plain / empty ────────────────────────────────

#[test]
fn plain_text_single_element() {
    let els = parse_inline_elements("hello world");
    assert_eq!(els.len(), 1);
    assert_eq!(as_str(&els[0], "type"), "text");
    assert_eq!(as_str(&els[0], "text"), "hello world");
    assert!(els[0].get("style").is_none());
}

#[test]
fn empty_input_yields_empty_text_element() {
    let els = parse_inline_elements("");
    assert_eq!(els.len(), 1);
    assert_eq!(as_str(&els[0], "type"), "text");
    assert_eq!(as_str(&els[0], "text"), "");
}

// ── bold / italic / code / strike ───────────────────────────────────────

#[test]
fn bold_span() {
    let els = parse_inline_elements("**strong**");
    assert_eq!(els.len(), 1);
    assert_eq!(as_str(&els[0], "text"), "strong");
    assert_eq!(els[0]["style"]["bold"], serde_json::json!(true));
}

#[test]
fn italic_span() {
    let els = parse_inline_elements("*emph*");
    assert_eq!(els.len(), 1);
    assert_eq!(as_str(&els[0], "text"), "emph");
    assert_eq!(els[0]["style"]["italic"], serde_json::json!(true));
}

#[test]
fn code_span() {
    let els = parse_inline_elements("`code`");
    assert_eq!(els.len(), 1);
    assert_eq!(as_str(&els[0], "text"), "code");
    assert_eq!(els[0]["style"]["code"], serde_json::json!(true));
}

#[test]
fn strike_span() {
    let els = parse_inline_elements("~~gone~~");
    assert_eq!(els.len(), 1);
    assert_eq!(as_str(&els[0], "text"), "gone");
    assert_eq!(els[0]["style"]["strike"], serde_json::json!(true));
}

#[test]
fn italic_with_interior_space_skip() {
    // Close preceded by a space is skipped, so the span extends.
    let els = parse_inline_elements("*a * b*");
    assert_eq!(els.len(), 1);
    assert_eq!(as_str(&els[0], "text"), "a * b");
    assert_eq!(els[0]["style"]["italic"], serde_json::json!(true));
}

#[test]
fn double_backtick_not_code() {
    // A doubled backtick is not treated as inline code; stays literal.
    let els = parse_inline_elements("``");
    assert_eq!(els.len(), 1);
    assert_eq!(as_str(&els[0], "text"), "``");
}

#[test]
fn mixed_inline_content() {
    let els = parse_inline_elements("a **b** c");
    assert_eq!(els.len(), 3);
    assert_eq!(as_str(&els[0], "text"), "a ");
    assert_eq!(as_str(&els[1], "text"), "b");
    assert_eq!(as_str(&els[2], "text"), " c");
}

// ── links ───────────────────────────────────────────────────────────────

#[test]
fn link_span() {
    let els = parse_inline_elements("[click](https://x.io)");
    assert_eq!(els.len(), 1);
    assert_eq!(as_str(&els[0], "type"), "link");
    assert_eq!(as_str(&els[0], "url"), "https://x.io");
    assert_eq!(as_str(&els[0], "text"), "click");
}

#[test]
fn link_surrounded_by_text() {
    let els = parse_inline_elements("see [t](u) now");
    assert_eq!(els.len(), 3);
    assert_eq!(as_str(&els[0], "text"), "see ");
    assert_eq!(as_str(&els[1], "type"), "link");
    assert_eq!(as_str(&els[2], "text"), " now");
}

#[test]
fn unbalanced_bracket_is_literal() {
    let els = parse_inline_elements("[abc");
    assert_eq!(els.len(), 1);
    assert_eq!(as_str(&els[0], "text"), "[abc");
}

#[test]
fn bracket_without_paren_is_literal() {
    let els = parse_inline_elements("[abc]xyz");
    assert_eq!(as_str(&els[0], "text"), "[abc]xyz");
}

// ── direct helper: try_parse_link ───────────────────────────────────────

fn cv(s: &str) -> Vec<char> {
    s.chars().collect()
}

#[test]
fn try_parse_link_valid() {
    let c = cv("[t](u)");
    let r = try_parse_link(&c, 0).unwrap();
    assert_eq!(r.0, "t");
    assert_eq!(r.1, "u");
    assert_eq!(r.2, 6);
}

#[test]
fn try_parse_link_not_bracket() {
    let c = cv("xyz");
    assert!(try_parse_link(&c, 0).is_none());
}

#[test]
fn try_parse_link_nested_brackets() {
    let c = cv("[a[b]c](u)");
    let r = try_parse_link(&c, 0).unwrap();
    assert_eq!(r.0, "a[b]c");
    assert_eq!(r.1, "u");
}

#[test]
fn try_parse_link_unbalanced_bracket() {
    let c = cv("[abc");
    assert!(try_parse_link(&c, 0).is_none());
}

#[test]
fn try_parse_link_no_open_paren() {
    let c = cv("[abc]x");
    assert!(try_parse_link(&c, 0).is_none());
}

#[test]
fn try_parse_link_unbalanced_paren() {
    let c = cv("[a](url");
    assert!(try_parse_link(&c, 0).is_none());
}

#[test]
fn try_parse_link_nested_parens() {
    let c = cv("[a](http://x(y))");
    let r = try_parse_link(&c, 0).unwrap();
    assert_eq!(r.0, "a");
    assert_eq!(r.1, "http://x(y)");
}

// ── direct helper: try_parse_delimited ──────────────────────────────────

#[test]
fn try_parse_delimited_valid() {
    let c = cv("`ab`");
    let r = try_parse_delimited(&c, 0, '`', '`', false).unwrap();
    assert_eq!(r.0, "ab");
    assert_eq!(r.1, 4);
}

#[test]
fn try_parse_delimited_not_open() {
    let c = cv("xab`");
    assert!(try_parse_delimited(&c, 0, '`', '`', false).is_none());
}

#[test]
fn try_parse_delimited_empty_inner() {
    let c = cv("``");
    assert!(try_parse_delimited(&c, 0, '`', '`', false).is_none());
}

#[test]
fn try_parse_delimited_newline_aborts() {
    let c = cv("`a\nb`");
    assert!(try_parse_delimited(&c, 0, '`', '`', false).is_none());
}

#[test]
fn try_parse_delimited_no_close() {
    let c = cv("`abc");
    assert!(try_parse_delimited(&c, 0, '`', '`', false).is_none());
}

#[test]
fn try_parse_delimited_skip_spaces_branch() {
    let c = cv("*a * b*");
    let r = try_parse_delimited(&c, 0, '*', '*', true).unwrap();
    assert_eq!(r.0, "a * b");
    assert_eq!(r.1, 7);
}

// ── direct helper: try_parse_double_delimited ───────────────────────────

#[test]
fn try_parse_double_valid() {
    let c = cv("**ab**");
    let r = try_parse_double_delimited(&c, 0, '*').unwrap();
    assert_eq!(r.0, "ab");
    assert_eq!(r.1, 6);
}

#[test]
fn try_parse_double_too_short() {
    let c = cv("*");
    assert!(try_parse_double_delimited(&c, 0, '*').is_none());
}

#[test]
fn try_parse_double_not_delim() {
    let c = cv("xy**");
    assert!(try_parse_double_delimited(&c, 0, '*').is_none());
}

#[test]
fn try_parse_double_empty_inner() {
    let c = cv("****");
    assert!(try_parse_double_delimited(&c, 0, '*').is_none());
}

#[test]
fn try_parse_double_blank_line_aborts() {
    let c = cv("**a\n\nb**");
    assert!(try_parse_double_delimited(&c, 0, '*').is_none());
}

#[test]
fn try_parse_double_no_close() {
    let c = cv("**abc");
    assert!(try_parse_double_delimited(&c, 0, '*').is_none());
}

// ── element constructors ────────────────────────────────────────────────

#[test]
fn text_element_all_styles() {
    assert!(text_element("x", None).get("style").is_none());
    assert_eq!(text_element("x", Some(InlineStyle::Bold))["style"]["bold"], serde_json::json!(true));
    assert_eq!(text_element("x", Some(InlineStyle::Italic))["style"]["italic"], serde_json::json!(true));
    assert_eq!(text_element("x", Some(InlineStyle::Code))["style"]["code"], serde_json::json!(true));
    assert_eq!(text_element("x", Some(InlineStyle::Strike))["style"]["strike"], serde_json::json!(true));
    let bi = text_element("x", Some(InlineStyle::BoldItalic));
    assert_eq!(bi["style"]["bold"], serde_json::json!(true));
    assert_eq!(bi["style"]["italic"], serde_json::json!(true));
}

#[test]
fn link_element_shape() {
    let l = link_element("http://a", "label");
    assert_eq!(as_str(&l, "type"), "link");
    assert_eq!(as_str(&l, "url"), "http://a");
    assert_eq!(as_str(&l, "text"), "label");
}

#[test]
fn unicode_content_preserved() {
    let els = parse_inline_elements("héllo 世界 🎉");
    assert_eq!(as_str(&els[0], "text"), "héllo 世界 🎉");
}
