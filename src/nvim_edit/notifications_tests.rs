use super::{decode, Notification};
use crate::nvim_ui::NvimNotification;
use rmpv::Value;

fn notification(method: &str, value: Value) -> NvimNotification {
    let mut params = Vec::new();
    rmpv::encode::write_value(&mut params, &value).expect("encode test notification");
    NvimNotification {
        method: method.into(),
        params,
    }
}

fn redraw(events: Vec<Value>) -> Vec<Notification> {
    decode(&notification("redraw", Value::Array(events)))
}

fn chunks(text: &str) -> Value {
    Value::Array(vec![Value::Array(vec![0.into(), text.into()])])
}

#[test]
fn lines_decode_to_incremental_ranges() {
    let result = decode(&notification(
        "nvim_buf_lines_event",
        Value::Array(vec![
            2.into(),
            9.into(),
            1.into(),
            2.into(),
            Value::Array(vec!["a".into(), "b".into()]),
            false.into(),
        ]),
    ));
    assert_eq!(
        result,
        vec![Notification::Lines {
            buffer: 2,
            changedtick: 9,
            first_line: 1,
            last_line: 2,
            new_last_line: 3,
            lines: vec!["a".into(), "b".into()],
        }]
    );
}

#[test]
fn initial_send_buffer_is_a_zero_length_replacement() {
    let result = decode(&notification(
        "nvim_buf_lines_event",
        Value::Array(vec![
            Value::Ext(0, vec![1]),
            4.into(),
            0.into(),
            (-1).into(),
            Value::Array(vec!["initial".into()]),
            false.into(),
        ]),
    ));
    assert!(matches!(
        result.as_slice(),
        [Notification::Lines {
            first_line: 0,
            last_line: 0,
            new_last_line: 1,
            lines,
            ..
        }] if lines == &["initial"]
    ));
}

#[test]
fn batched_messages_expand_to_one_notification_each() {
    let result = redraw(vec![Value::Array(vec![
        "msg_show".into(),
        Value::Array(vec!["echo".into(), chunks("hi"), false.into()]),
        Value::Array(vec!["emsg".into(), chunks("bad"), false.into()]),
    ])]);
    assert_eq!(
        result,
        vec![
            Notification::Message {
                kind: "echo".into(),
                text: "hi".into()
            },
            Notification::Message {
                kind: "emsg".into(),
                text: "bad".into()
            },
        ]
    );
}

#[test]
fn unbatched_messages_are_still_accepted() {
    let result = redraw(vec![Value::Array(vec![
        "msg_show".into(),
        "info".into(),
        Value::Array(vec![
            Value::Array(vec![1.into(), "hello ".into()]),
            Value::Array(vec![2.into(), "world".into()]),
        ]),
        false.into(),
    ])]);
    assert_eq!(
        result,
        vec![Notification::Message {
            kind: "info".into(),
            text: "hello world".into()
        }]
    );
}

#[test]
fn the_command_line_is_neovims_own_state() {
    let result = redraw(vec![
        Value::Array(vec![
            "cmdline_show".into(),
            Value::Array(vec![
                chunks("echo \"hi\""),
                9.into(),
                ":".into(),
                "".into(),
                0.into(),
                1.into(),
            ]),
        ]),
        Value::Array(vec![
            "cmdline_pos".into(),
            Value::Array(vec![4.into(), 1.into()]),
        ]),
        Value::Array(vec!["flush".into(), Value::Array(Vec::new())]),
    ]);
    assert_eq!(
        result,
        vec![
            Notification::CmdlineShow {
                content: "echo \"hi\"".into(),
                position: 9,
                first_char: ":".into(),
            },
            Notification::CmdlinePos { position: 4 },
            Notification::Flush,
        ]
    );
    assert_eq!(
        redraw(vec![Value::Array(vec!["cmdline_hide".into()])]),
        vec![Notification::CmdlineHide]
    );
}

#[test]
fn message_history_flattens_to_one_block() {
    let result = redraw(vec![Value::Array(vec![
        "msg_history_show".into(),
        Value::Array(vec![Value::Array(vec![
            Value::Array(vec!["echo".into(), chunks("hi")]),
            Value::Array(vec!["emsg".into(), chunks("bad")]),
        ])]),
    ])]);
    assert_eq!(
        result,
        vec![Notification::Message {
            kind: "history".into(),
            text: "hi\nbad".into()
        }]
    );
}

#[test]
fn neovim_keymaps_reach_opman_as_actions() {
    assert_eq!(
        decode(&notification(
            "opman_action",
            Value::Array(vec!["lsp.definition".into()])
        )),
        vec![Notification::Action {
            name: "lsp.definition".into()
        }]
    );
}
