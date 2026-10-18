use super::*;

#[test]
fn finds_the_script_line_in_a_stack() {
    let stack =
        "    at f (prelude:3:1)\n    at <anonymous> (execute:7:12)\n    at <eval> (execute:9:3)\n";
    assert_eq!(script_line(stack), Some(7));
    assert_eq!(script_line("    at x (prelude:1:1)"), None);
    assert_eq!(script_line(""), None);
}

#[test]
fn display_includes_the_line_only_when_known() {
    let with = Failure {
        name: "TypeError".into(),
        message: "bad".into(),
        line: Some(2),
    };
    assert_eq!(with.to_string(), "TypeError: bad (line 2)");
    let without = Failure {
        name: "Error".into(),
        message: "bad".into(),
        line: None,
    };
    assert_eq!(without.to_string(), "Error: bad");
}
