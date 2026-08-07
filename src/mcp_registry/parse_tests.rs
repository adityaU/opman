//! Placeholder parsing. The rule these all protect: a config value that merely *looks*
//! like a placeholder must reach the runner unchanged rather than silently lose text.

use super::*;
use crate::mcp_registry::spec::Arg;

#[test]
fn plain_text_is_one_literal() {
    assert_eq!(arg("--flag", "s"), Arg::lit("--flag"));
}

#[test]
fn a_lone_placeholder_stays_flat() {
    assert_eq!(arg("${dir}", "s"), Arg::Dir);
    assert_eq!(arg("${session}", "s"), Arg::SessionId);
    assert_eq!(arg("${env:HOME}", "s"), Arg::Env("HOME".into()));
}

#[test]
fn embedded_placeholders_become_mixed() {
    let Arg::Mixed(parts) = arg("--data-dir=${dir}/.cache", "s") else {
        panic!("expected Mixed");
    };
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], Arg::lit("--data-dir="));
    assert_eq!(parts[1], Arg::Dir);
    assert_eq!(parts[2], Arg::lit("/.cache"));
}

#[test]
fn two_placeholders_in_one_value() {
    let Arg::Mixed(parts) = arg("${dir}:${session}", "s") else {
        panic!("expected Mixed");
    };
    assert_eq!(parts[0], Arg::Dir);
    assert_eq!(parts[1], Arg::lit(":"));
    assert_eq!(parts[2], Arg::SessionId);
}

#[test]
fn a_bare_dollar_is_literal() {
    assert_eq!(arg("cost: $5", "s"), Arg::lit("cost: $5"));
}

#[test]
fn an_unknown_placeholder_is_kept_verbatim() {
    // Losing the text would be worse than passing it through: the server may well
    // expand it itself.
    assert_eq!(arg("${nope}", "s"), Arg::lit("${nope}"));
    assert_eq!(arg("a${nope}b", "s"), Arg::lit("a${nope}b"));
}

#[test]
fn an_unterminated_placeholder_is_literal() {
    assert_eq!(arg("${dir", "s"), Arg::lit("${dir"));
}

#[test]
fn an_empty_env_name_is_not_a_placeholder() {
    assert_eq!(arg("${env:}", "s"), Arg::lit("${env:}"));
}

#[test]
fn empty_input_yields_an_empty_literal() {
    assert_eq!(arg("", "s"), Arg::lit(""));
}
