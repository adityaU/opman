//! Cold-path conversions for the `rmpv::Value` API.

use rmpv::Value;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueError {
    ExpectedInteger,
    MalformedExt,
    NonUtf8String,
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ExpectedInteger => "expected an integer value",
            Self::MalformedExt => "malformed MessagePack Ext integer",
            Self::NonUtf8String => "MessagePack string is not valid UTF-8",
        })
    }
}

impl std::error::Error for ValueError {}

pub fn ext_or_int(value: &Value) -> Result<i64, ValueError> {
    match value {
        Value::Integer(integer) => integer.as_i64().ok_or(ValueError::ExpectedInteger),
        Value::Ext(_, data) => {
            let mut input = &data[..];
            let decoded =
                rmpv::decode::read_value(&mut input).map_err(|_| ValueError::MalformedExt)?;
            if !input.is_empty() {
                return Err(ValueError::MalformedExt);
            }
            decoded.as_i64().ok_or(ValueError::MalformedExt)
        }
        _ => Err(ValueError::ExpectedInteger),
    }
}

pub fn value_to_string(value: &Value) -> Result<String, ValueError> {
    match value {
        Value::String(string) => string
            .as_str()
            .ok_or(ValueError::NonUtf8String)
            .map(str::to_owned),
        Value::Nil => Ok(String::new()),
        Value::Boolean(value) => Ok(value.to_string()),
        Value::Integer(value) => Ok(value.to_string()),
        Value::F32(value) => Ok(value.to_string()),
        Value::F64(value) => Ok(value.to_string()),
        other => Ok(other.to_string()),
    }
}

#[cfg(test)]
#[path = "value_tests.rs"]
mod value_tests;
