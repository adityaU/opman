use crate::nvim_ui::stream::wire::{ClientMsg, ExCommand};

#[test]
fn structured_commands_decode_without_a_method_field() {
    let message: ClientMsg = serde_json::from_str(
        r#"{"type":"command","command":{"command":"substitute","pattern":"foo","replacement":"bar","global":true,"ignore_case":false}}"#,
    )
    .expect("closed structured command");
    assert!(matches!(
        message,
        ClientMsg::Command {
            command: ExCommand::Substitute { .. }
        }
    ));
}

#[test]
fn arbitrary_rpc_methods_and_free_form_commands_are_rejected() {
    for text in [
        r#"{"type":"nvim_eval","expr":"vim.version()"}"#,
        r#"{"type":"input","keys":"x","method":"nvim_eval"}"#,
        r#"{"type":"command","command":":%s/a/b/g"}"#,
    ] {
        assert!(serde_json::from_str::<ClientMsg>(text).is_err());
    }
}
