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
