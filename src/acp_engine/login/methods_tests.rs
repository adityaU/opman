use serde_json::json;

use super::*;

fn list() -> Vec<Method> {
    parse(&json!({ "authMethods": [
        { "id": "oauth", "name": "Log in with Google" },
        { "id": "key", "name": "API key" },
    ]}))
}

#[test]
fn reads_names_and_falls_back_to_the_id() {
    let methods = parse(&json!({ "authMethods": [{ "id": "a" }, { "id": "b", "name": "B" }] }));
    assert_eq!(methods[0].name, "a");
    assert_eq!(methods[1].name, "B");
    assert_eq!(methods[0].kind, Kind::Agent);
}

#[test]
fn entries_without_an_id_are_skipped() {
    let methods = parse(&json!({ "authMethods": [{ "name": "x" }, { "id": "" }, { "id": "ok" }] }));
    assert_eq!(methods.len(), 1);
}

#[test]
fn a_terminal_method_keeps_its_args_and_env() {
    let methods = parse(&json!({ "authMethods": [{
        "id": "t", "type": "terminal", "args": ["--login"], "env": { "A": "1", "B": 2 }
    }]}));
    assert_eq!(
        methods[0].kind,
        Kind::Terminal {
            args: vec!["--login".into()],
            env: vec![("A".into(), "1".into())],
        }
    );
}

#[test]
fn no_methods_is_an_error() {
    assert!(choose(&[], None).is_err());
    assert!(parse(&json!({})).is_empty());
}

#[test]
fn the_only_method_is_used_and_several_ask() {
    let all = list();
    assert_eq!(choose(&all[..1], None), Ok(Choice::Use(&all[0])));
    assert_eq!(choose(&all, None), Ok(Choice::Ask));
}

#[test]
fn a_named_method_must_exist() {
    let all = list();
    assert_eq!(choose(&all, Some("key")), Ok(Choice::Use(&all[1])));
    let error = choose(&all, Some("nope")).expect_err("unknown id");
    assert!(error.contains("oauth, key"), "{error}");
}

#[test]
fn answers_are_numbers_and_enter_means_the_first() {
    let all = list();
    assert_eq!(by_number(&all, "2\n"), Some(&all[1]));
    assert_eq!(by_number(&all, "\n"), Some(&all[0]));
    assert_eq!(by_number(&all, "0"), None);
    assert_eq!(by_number(&all, "3"), None);
    assert_eq!(by_number(&all, "x"), None);
}
