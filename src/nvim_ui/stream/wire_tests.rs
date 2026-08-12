use super::{ClientMsg, ControlMsg, ModeShort, NvimMode};
use serde_json::json;

#[test]
fn control_messages_use_snake_case_type_tags() {
    assert_eq!(
        serde_json::to_value(ControlMsg::Ready {}).unwrap(),
        json!({"type": "ready"})
    );
    assert_eq!(
        serde_json::to_value(ControlMsg::InputAck {}).unwrap(),
        json!({"type": "input_ack"})
    );
    assert_eq!(
        serde_json::to_value(ControlMsg::Error {
            message: "bad".into()
        })
        .unwrap(),
        json!({"type": "error", "message": "bad"})
    );
    assert_eq!(
        serde_json::to_value(ControlMsg::Exited { code: Some(17) }).unwrap(),
        json!({"type": "exited", "code": 17})
    );
    assert_eq!(
        serde_json::to_value(ControlMsg::Superseded {}).unwrap(),
        json!({"type": "superseded"})
    );
    assert_eq!(
        serde_json::to_value(ControlMsg::TooSlow {}).unwrap(),
        json!({"type": "too_slow"})
    );
}

#[test]
fn client_messages_round_trip_without_an_opcode() {
    let messages = [
        ClientMsg::Input {
            keys: "ihello".into(),
        },
        ClientMsg::InputMouse {
            button: "left".into(),
            action: "press".into(),
            modifier: "".into(),
            grid: 0,
            row: 4,
            col: 8,
        },
        ClientMsg::Resize { rows: 24, cols: 80 },
        ClientMsg::Paste {
            data: "hello\n".into(),
        },
    ];

    for message in messages {
        let encoded = serde_json::to_string(&message).unwrap();
        assert!(!encoded.contains("method"));
        let decoded: ClientMsg = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, message);
    }
}

#[test]
fn unknown_client_variants_and_rpc_methods_are_rejected() {
    for input in [
        r#"{"type":"nvim_eval","method":"nvim_eval","expr":"vim.version()"}"#,
        r#"{"type":"arbitrary_rpc","method":"nvim_command","params":[]}"#,
        r#"{"type":"input","keys":"x","method":"nvim_input"}"#,
    ] {
        assert!(
            serde_json::from_str::<ClientMsg>(input).is_err(),
            "accepted {input}"
        );
    }
}

#[test]
fn unknown_control_variants_are_rejected() {
    assert!(serde_json::from_str::<ControlMsg>(r#"{"type":"rpc_result"}"#).is_err());
    assert!(serde_json::from_str::<ControlMsg>(r#"{"type":"ready","method":"x"}"#).is_err());
}

#[test]
fn nvim_mode_codes_are_closed_and_have_semantic_short_names() {
    assert_eq!(NvimMode::try_from("i").unwrap().short(), ModeShort::Insert);
    assert_eq!(NvimMode::try_from("R").unwrap().short(), ModeShort::Replace);
    assert_eq!(
        NvimMode::try_from("V").unwrap().short(),
        ModeShort::VisualLine
    );
    assert_eq!(
        NvimMode::try_from("\u{16}").unwrap().short(),
        ModeShort::VisualBlock
    );
    assert!(NvimMode::try_from("not-a-neovim-mode").is_err());
    assert!(serde_json::from_str::<NvimMode>(r#""bogus""#).is_err());
}
