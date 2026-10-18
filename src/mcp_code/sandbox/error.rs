//! A script failure, in the shape a JavaScript developer expects.

use std::fmt;

use rquickjs::{CaughtError, Function};

/// File name the script is compiled under; stack frames carry it.
pub const SCRIPT_NAME: &str = "execute";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// `TypeError`, `SyntaxError`, `Error`, or `Uncaught` for a thrown non-error.
    pub name: String,
    pub message: String,
    /// 1-based line in the submitted code, when the engine reported one.
    pub line: Option<u32>,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.message)?;
        if let Some(line) = self.line {
            write!(f, " (line {line})")?;
        }
        Ok(())
    }
}

impl Failure {
    /// Read a caught exception. `render` prints thrown non-error values.
    pub fn from_caught<'js>(caught: CaughtError<'js>, render: &Function<'js>) -> Self {
        match caught {
            CaughtError::Exception(exception) => {
                let name = exception
                    .get::<_, String>("name")
                    .unwrap_or_else(|_| "Error".into());
                Self {
                    name,
                    message: exception.message().unwrap_or_default(),
                    line: exception.stack().as_deref().and_then(script_line),
                }
            }
            CaughtError::Value(value) => {
                let text = render.call::<_, String>((value,)).unwrap_or_default();
                Self {
                    name: "Uncaught".into(),
                    message: text,
                    line: None,
                }
            }
            CaughtError::Error(error) => Self {
                name: "Error".into(),
                message: error.to_string(),
                line: None,
            },
        }
    }

    /// For failures that happen before any script value exists.
    pub fn internal(error: rquickjs::Error) -> Self {
        Self {
            name: "Error".into(),
            message: error.to_string(),
            line: None,
        }
    }
}

/// The first `execute:LINE:COL` frame in a QuickJS stack.
fn script_line(stack: &str) -> Option<u32> {
    let marker = format!("{SCRIPT_NAME}:");
    let at = stack.find(&marker)? + marker.len();
    let digits: String = stack[at..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;
