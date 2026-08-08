use super::*;

#[test]
fn output_under_the_limit_is_kept_whole() {
    let mut buffer = Buffer::new(64);
    buffer.push(b"hello ");
    buffer.push(b"world");
    assert_eq!(buffer.text(), "hello world");
    assert!(!buffer.truncated());
}

/// The protocol has the client truncate from the *beginning*, so what survives is the most
/// recent output — the part a command's reader actually wants.
#[test]
fn the_tail_survives_truncation() {
    let mut buffer = Buffer::new(8);
    buffer.push(b"0123456789");
    assert_eq!(buffer.text(), "23456789");
    assert!(buffer.truncated());
}

/// Cutting a multi-byte character in half would corrupt the whole tail, not just the byte it
/// landed on, so the drop advances to the next character boundary even at the cost of a
/// slightly smaller buffer than asked for.
#[test]
fn truncation_lands_on_a_character_boundary() {
    let mut buffer = Buffer::new(4);
    // Six bytes: two three-byte characters. Keeping four would split the first.
    buffer.push("日本".as_bytes());
    assert_eq!(buffer.text(), "本");
    assert!(buffer.truncated());
}

/// A buffer trimmed repeatedly must stay valid every time, not only on the first pass.
#[test]
fn repeated_truncation_stays_valid_utf8() {
    let mut buffer = Buffer::new(5);
    for _ in 0..4 {
        buffer.push("héllo".as_bytes());
    }
    assert!(buffer.text().chars().all(|c| "héllo".contains(c)));
}

/// ACP names the signal rather than numbering it.
#[test]
fn a_killed_command_reports_its_signal_by_name() {
    let exit = Exit {
        code: None,
        signal: Some(9),
    };
    let value = exit.to_value();
    assert_eq!(value["signal"], "SIGKILL");
    assert!(value["exitCode"].is_null());
}

#[test]
fn a_normal_exit_reports_its_code_and_no_signal() {
    let exit = Exit {
        code: Some(0),
        signal: None,
    };
    let value = exit.to_value();
    assert_eq!(value["exitCode"], 0);
    assert!(value["signal"].is_null());
}

/// An unnamed signal is still worth reporting by number: the agent can look it up, where a
/// dropped field tells it nothing.
#[test]
fn unnamed_signals_are_reported_by_number() {
    let exit = Exit {
        code: None,
        signal: Some(31),
    };
    assert_eq!(exit.to_value()["signal"], "31");
}
