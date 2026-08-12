use super::*;
use std::str::FromStr;

#[test]
fn every_wire_name_parses_and_round_trips() {
    assert_eq!(NvimOp::ALL.len(), 21);
    for expected in NvimOp::ALL {
        let parsed = NvimOp::from_str(expected.as_str()).expect("known op");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.to_string(), expected.as_str());
    }
}

#[test]
fn unknown_names_are_rejected() {
    for name in ["", "nvim_bogus", "nvim_command ", "NvimEval"] {
        assert!(NvimOp::from_str(name).is_err(), "{name} must be rejected");
    }
}

#[test]
fn capability_tiers_are_closed_and_exact() {
    let execute = [NvimOp::Command, NvimOp::Eval];
    let edit = [
        NvimOp::Open,
        NvimOp::Input,
        NvimOp::Write,
        NvimOp::EditAndSave,
        NvimOp::Undo,
        NvimOp::Rename,
        NvimOp::Format,
    ];
    let read = [
        NvimOp::Read,
        NvimOp::Buffers,
        NvimOp::Info,
        NvimOp::Diagnostics,
        NvimOp::Definition,
        NvimOp::References,
        NvimOp::Hover,
        NvimOp::Symbols,
        NvimOp::CodeActions,
        NvimOp::Grep,
        NvimOp::Diff,
        NvimOp::Signature,
    ];
    assert!(execute
        .iter()
        .all(|op| op.capability() == Capability::Execute));
    assert!(edit.iter().all(|op| op.capability() == Capability::Edit));
    assert!(read.iter().all(|op| op.capability() == Capability::Read));
    assert_eq!(execute.len() + edit.len() + read.len(), NvimOp::ALL.len());
}

#[test]
fn execute_operations_are_explicit() {
    assert_eq!(NvimOp::Eval.capability(), Capability::Execute);
    assert_eq!(NvimOp::Command.capability(), Capability::Execute);
}
