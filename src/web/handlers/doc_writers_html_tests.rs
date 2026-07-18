//! Generated tests for `doc_writers_html.rs` (sanitize + block/run parsing).
use super::*;

#[test]
fn sanitize_keeps_allowed_strips_disallowed() {
    let input = "<p>hi <b>bold</b> <script>evil()</script> <span>s</span></p>";
    let out = sanitize_html(input);
    assert!(out.contains("<p>"), "got {out}");
    assert!(out.contains("<b>"));
    assert!(out.contains("</b>"));
    assert!(out.contains("<span>"));
    // Disallowed tag stripped, but inner text stays.
    assert!(!out.contains("<script>"));
    assert!(out.contains("evil()"));
    assert!(out.contains("hi "));
}

#[test]
fn sanitize_self_closing_and_close_tags() {
    let out = sanitize_html("line<br/>next</p>");
    assert!(out.contains("<br/>"), "got {out}");
    assert!(out.contains("</p>"));
}

#[test]
fn sanitize_strips_attributes_from_allowed_tags() {
    let out = sanitize_html("<p class=\"x\" onclick=\"y\">t</p>");
    assert!(out.contains("<p>t</p>"), "got {out}");
    assert!(!out.contains("class"));
    assert!(!out.contains("onclick"));
}

#[test]
fn sanitize_dangling_lt_without_gt_is_literal() {
    // A `<` with no closing `>` is emitted as a literal char.
    let out = sanitize_html("a < b");
    assert!(out.contains('<'), "got {out}");
    assert!(out.contains("b"));
}

#[test]
fn sanitize_disallowed_close_tag_stripped() {
    let out = sanitize_html("</script>text");
    assert!(!out.contains("script"), "got {out}");
    assert!(out.contains("text"));
}

#[test]
fn parse_blocks_block_tags_and_headings() {
    let blocks = parse_blocks("<h1>Title</h1><p>body</p><li>item</li><div>d</div>");
    assert_eq!(blocks.len(), 4);
    assert_eq!(blocks[0].heading_level, Some(1));
    assert_eq!(blocks[0].text, "Title");
    assert_eq!(blocks[1].heading_level, None);
    assert_eq!(blocks[1].text, "body");
    assert_eq!(blocks[2].text, "item");
    assert_eq!(blocks[3].text, "d");
}

#[test]
fn parse_blocks_all_heading_levels() {
    let blocks =
        parse_blocks("<h1>a</h1><h2>b</h2><h3>c</h3><h4>d</h4><h5>e</h5><h6>f</h6>");
    let levels: Vec<_> = blocks.iter().map(|b| b.heading_level).collect();
    assert_eq!(
        levels,
        vec![Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)]
    );
}

#[test]
fn parse_blocks_prefix_text_before_tag() {
    // Text appearing before the first block tag becomes its own block.
    let blocks = parse_blocks("loose text<p>inside</p>");
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].text, "loose text");
    assert_eq!(blocks[0].heading_level, None);
    assert_eq!(blocks[1].text, "inside");
}

#[test]
fn parse_blocks_runs_inside_block() {
    let blocks = parse_blocks("<p><b>B</b>x</p>");
    assert_eq!(blocks.len(), 1);
    let runs = &blocks[0].runs;
    assert_eq!(runs.len(), 2);
    assert!(runs[0].bold);
    assert_eq!(runs[0].text, "B");
    assert!(!runs[1].bold);
    assert_eq!(runs[1].text, "x");
}

#[test]
fn parse_blocks_unclosed_block_tag_falls_through() {
    // No matching close tag -> block branch skipped, advances past the tag.
    let blocks = parse_blocks("<p>never closed");
    // "never closed" becomes trailing text block.
    assert!(blocks.iter().any(|b| b.text.contains("never closed")), "{:?}", blocks.iter().map(|b| &b.text).collect::<Vec<_>>());
}

#[test]
fn parse_blocks_closing_tag_first_is_skipped() {
    let blocks = parse_blocks("</p><p>real</p>");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].text, "real");
}

#[test]
fn parse_blocks_non_block_tag_advances() {
    // A span (inline, not a block tag) is not treated as a block open.
    let blocks = parse_blocks("<span>x</span><p>y</p>");
    // The <span> content is captured as trailing/inter text or ignored; the <p> is a block.
    assert!(blocks.iter().any(|b| b.text == "y"));
}

#[test]
fn parse_blocks_trailing_text() {
    let blocks = parse_blocks("<p>a</p>tail");
    assert_eq!(blocks.last().unwrap().text, "tail");
}

#[test]
fn parse_runs_all_formats_toggle() {
    let runs = parse_runs("<b>bold</b><i>ital</i><u>und</u><s>strike</s>plain");
    assert_eq!(runs.len(), 5);
    assert!(runs[0].bold && !runs[0].italic);
    assert!(runs[1].italic);
    assert!(runs[2].underline);
    assert!(runs[3].strike);
    let last = &runs[4];
    assert!(!last.bold && !last.italic && !last.underline && !last.strike);
    assert_eq!(last.text, "plain");
}

#[test]
fn parse_runs_strong_em_del_aliases() {
    let runs = parse_runs("<strong>a</strong><em>b</em><del>c</del>");
    assert!(runs[0].bold);
    assert!(runs[1].italic);
    assert!(runs[2].strike);
}

#[test]
fn parse_runs_nested_formatting() {
    let runs = parse_runs("<b><i>bi</i></b>");
    assert_eq!(runs.len(), 1);
    assert!(runs[0].bold && runs[0].italic);
    assert_eq!(runs[0].text, "bi");
}

#[test]
fn parse_runs_unterminated_tag_kept_literal() {
    // A `<` with no `>` before end is pushed as a literal char. The leading
    // text is flushed as its own run, so the literal `<` lands in a later run.
    let runs = parse_runs("text<");
    assert!(runs.iter().any(|r| r.text.contains('<')));
    assert_eq!(runs.iter().map(|r| r.text.as_str()).collect::<String>(), "text<");
}

#[test]
fn parse_runs_empty_input() {
    assert!(parse_runs("").is_empty());
}

#[test]
fn strip_inline_tags_removes_tags() {
    assert_eq!(strip_inline_tags("<b>x</b>y<i>z</i>"), "xyz");
    assert_eq!(strip_inline_tags("plain"), "plain");
    assert_eq!(strip_inline_tags(""), "");
}

#[test]
fn html_unescape_entities() {
    assert_eq!(
        html_unescape("a &amp; b &lt; c &gt; d &quot;e&quot; &#39;f&#39; g&nbsp;h"),
        "a & b < c > d \"e\" 'f' g h"
    );
    assert_eq!(html_unescape("none"), "none");
}

#[test]
fn is_allowed_tag_via_sanitize_covers_many() {
    // Exercise a representative set of allowed tags end-to-end.
    let input = "<h6>x</h6><ol><li>i</li></ol><table><thead><tr><th>t</th></tr></thead>\
                 <tbody><tr><td>d</td></tr></tbody></table><em>e</em><strong>s</strong><del>x</del>";
    let out = sanitize_html(input);
    for t in ["<h6>", "<ol>", "<li>", "<table>", "<thead>", "<tr>", "<th>", "<tbody>", "<td>", "<em>", "<strong>", "<del>"] {
        assert!(out.contains(t), "missing {t} in {out}");
    }
}
