use super::*;

#[test]
fn malformed_ext_is_an_error_not_buffer_zero() {
    assert_eq!(
        ext_or_int(&rmpv::Value::Ext(1, vec![0xc1])),
        Err(ValueError::MalformedExt)
    );
    assert_eq!(
        ext_or_int(&rmpv::Value::Ext(1, vec![])),
        Err(ValueError::MalformedExt)
    );
    assert_eq!(ext_or_int(&rmpv::Value::Ext(1, vec![0x2a])), Ok(42));
}

#[test]
fn non_utf8_string_is_an_error_not_empty_string() {
    let mut input = &[0xa1, 0xff][..];
    let value = rmpv::decode::read_value(&mut input).unwrap();
    assert!(value_to_string(&value).is_err());
}

#[test]
fn scalar_values_are_readable() {
    assert_eq!(value_to_string(&rmpv::Value::Nil), Ok(String::new()));
    assert_eq!(
        value_to_string(&rmpv::Value::Boolean(true)),
        Ok(String::from("true"))
    );
    assert_eq!(
        value_to_string(&rmpv::Value::from(12i64)),
        Ok(String::from("12"))
    );
    assert_eq!(ext_or_int(&rmpv::Value::from(9i64)), Ok(9));
    assert_eq!(
        ext_or_int(&rmpv::Value::Nil),
        Err(ValueError::ExpectedInteger)
    );
}
