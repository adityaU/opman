use super::html::to_text;

#[test]
fn keeps_headings_links_lists_and_code() {
    let page = "<html><head><title>x</title><style>p{}</style></head><body>\
        <h1>Title</h1><p>Hello <a href=\"https://a.dev/x\">link</a> &amp; <b>bold</b>.</p>\
        <ul><li>one</li><li>two</li></ul><pre>let a = 1;\n  b</pre>\
        <script>alert(1)</script></body></html>";
    let text = to_text(page);
    assert!(text.contains("# Title"), "{text}");
    assert!(
        text.contains("[link](https://a.dev/x) & **bold**."),
        "{text}"
    );
    assert!(text.contains("- one\n- two"), "{text}");
    assert!(text.contains("let a = 1;\n  b"), "{text}");
    assert!(!text.contains("alert"), "{text}");
    assert!(!text.contains("p{}"), "{text}");
}

#[test]
fn decodes_entities_and_drops_empty_links() {
    let text = to_text("<p>a&nbsp;b &#65;&#x42; &bogus; <a href=\"#top\">top</a></p><!-- c -->");
    assert!(text.contains("a b AB &bogus; top"), "{text}");
    assert!(!text.contains('['), "{text}");
}
