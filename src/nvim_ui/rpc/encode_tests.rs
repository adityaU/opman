use super::*;
use rmpv::Value;

fn decode(bytes: &[u8]) -> Value {
    let mut input = bytes;
    let value = rmpv::decode::read_value(&mut input).unwrap();
    assert!(input.is_empty());
    value
}

#[test]
fn input_round_trips_as_notification() {
    let encoded = encode_nvim_input("ihello<Esc>").unwrap();
    assert_eq!(
        decode(&encoded),
        Value::Array(vec![
            Value::from(2u64),
            Value::from("nvim_input"),
            Value::Array(vec![Value::from("ihello<Esc>")])
        ])
    );
}

#[test]
fn mouse_round_trips_with_signed_coordinates() {
    let encoded = encode_nvim_input_mouse("left", "drag", "S", 1, -2, 300).unwrap();
    assert_eq!(
        decode(&encoded),
        Value::Array(vec![
            Value::from(2u64),
            Value::from("nvim_input_mouse"),
            Value::Array(vec![
                Value::from("left"),
                Value::from("drag"),
                Value::from("S"),
                Value::from(1i64),
                Value::from(-2i64),
                Value::from(300i64)
            ])
        ])
    );
}

#[test]
fn resize_round_trips_without_a_msgid() {
    let encoded = encode_nvim_ui_try_resize(120, 40).unwrap();
    assert_eq!(
        decode(&encoded),
        Value::Array(vec![
            Value::from(2u64),
            Value::from("nvim_ui_try_resize"),
            Value::Array(vec![Value::from(120u64), Value::from(40u64)])
        ])
    );
}

#[test]
fn paste_uses_false_and_minus_one_phase() {
    let encoded = encode_nvim_paste("one\ntwo").unwrap();
    assert_eq!(
        decode(&encoded),
        Value::Array(vec![
            Value::from(2u64),
            Value::from("nvim_paste"),
            Value::Array(vec![
                Value::from("one\ntwo"),
                Value::Boolean(false),
                Value::from(-1i64)
            ])
        ])
    );
}

#[test]
fn into_variants_append_directly() {
    let mut buffer = Vec::new();
    encode_nvim_input_into(&mut buffer, "a").unwrap();
    let first_len = buffer.len();
    encode_nvim_ui_try_resize_into(&mut buffer, 80, 24).unwrap();
    let mut input = &buffer[..first_len];
    assert!(rmpv::decode::read_value(&mut input).is_ok());
    let mut rest = &buffer[first_len..];
    assert!(rmpv::decode::read_value(&mut rest).is_ok());
    assert!(rest.is_empty());
}
