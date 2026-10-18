use super::*;
use crate::mcp_code::catalog::tests_support::sample;

#[test]
fn small_catalog_is_listed_in_full() {
    let (text, listing) = build(&sample(), DEFAULT_BUDGET);
    assert_eq!(listing, Listing::Full);
    assert!(text.contains("## Workflow"));
    assert!(text.contains("## Rules"));
    assert!(text.contains("## Available tools"));
    assert!(text.contains("await tools.fake.add(input: {"));
    assert!(text.contains("Promise.all"));
    assert!(text.contains("Object.keys(tools)"));
}

#[test]
fn tight_budget_falls_back_to_compact_then_search_only() {
    let catalog = sample();
    let (full, _) = build(&catalog, DEFAULT_BUDGET);
    let full_len = full.len();

    let full_body = render(&catalog, Style::Full);
    let compact_body = render(&catalog, Style::Compact);
    assert!(compact_body.len() < full_body.len());
    let (compact, listing) = build(&catalog, compact_body.len());
    assert_eq!(listing, Listing::Compact, "{compact}");
    assert!(compact.len() < full_len);

    let (search, listing) = build(&catalog, 10);
    assert_eq!(listing, Listing::SearchOnly);
    assert!(search.contains("tools.$codemode.search"));
    assert!(search.contains("- fake (6 tools)"));
    assert!(!search.contains("await tools.fake.add("));
}

#[test]
fn unavailable_servers_are_named() {
    let mut catalog = sample();
    catalog
        .unavailable
        .push(crate::mcp_code::catalog::Unavailable {
            server: "broken".into(),
            reason: "did not start".into(),
        });
    let (text, _) = build(&catalog, DEFAULT_BUDGET);
    assert!(text.contains("- broken: did not start"));
}

#[test]
fn empty_catalog_says_so() {
    let (text, _) = build(&Catalog::default(), DEFAULT_BUDGET);
    assert!(text.contains("(no tools are available)"));
}
