//! Closed Ex command mapping. User text is data, never a command string.

use anyhow::{bail, Result};
use rmpv::Value;
use std::str::FromStr;

use crate::mcp::{Capability, NvimOp};
use crate::nvim_ui::stream::wire::ExCommand;

pub(crate) fn open_request(path: &str) -> Result<Value> {
    authorize_operation(NvimOp::Open, Capability::Edit)?;
    Ok(Value::Array(vec![
        Value::Map(vec![
            (Value::from("cmd"), Value::from("edit")),
            (Value::from("args"), Value::Array(vec![Value::from(path)])),
        ]),
        Value::Map(vec![(Value::from("output"), Value::Boolean(true))]),
    ]))
}

pub(crate) fn request(command: &ExCommand, line_count: Option<usize>) -> Result<Value> {
    authorize()?;
    let (name, args, range, bang) = match command {
        ExCommand::Write => ("write", Vec::new(), None, false),
        ExCommand::WriteAll => ("wall", Vec::new(), None, false),
        ExCommand::Quit => ("quit", Vec::new(), None, false),
        ExCommand::ForceQuit => ("quit", Vec::new(), None, true),
        ExCommand::BufferDelete => ("bdelete", Vec::new(), None, false),
        ExCommand::NoHighlight => ("nohlsearch", Vec::new(), None, false),
        ExCommand::EditReload => ("edit", Vec::new(), None, true),
        ExCommand::Undo => ("undo", Vec::new(), None, false),
        ExCommand::Redo => ("redo", Vec::new(), None, false),
        ExCommand::Substitute {
            pattern,
            replacement,
            global,
            ignore_case,
        } => {
            let mut flags = String::new();
            if *global {
                flags.push('g');
            }
            if *ignore_case {
                flags.push('i');
            }
            (
                "substitute",
                vec![substitute_arg(pattern, replacement, &flags)],
                line_count.map(|count| (1, count.max(1))),
                false,
            )
        }
        ExCommand::Sort {
            reverse,
            numeric,
            unique,
            ignore_case,
        } => {
            let mut flags = String::new();
            if *ignore_case {
                flags.push('i');
            }
            if *numeric {
                flags.push('n');
            }
            if *unique {
                flags.push('u');
            }
            let args = if flags.is_empty() {
                Vec::new()
            } else {
                vec![flags]
            };
            (
                "sort",
                args,
                line_count.map(|count| (1, count.max(1))),
                *reverse,
            )
        }
    };
    let mut command_map = vec![(Value::from("cmd"), Value::from(name))];
    if !args.is_empty() {
        command_map.push((
            Value::from("args"),
            Value::Array(args.into_iter().map(Value::from).collect()),
        ));
    }
    if let Some((start, end)) = range {
        command_map.push((
            Value::from("range"),
            Value::Array(vec![Value::from(start), Value::from(end)]),
        ));
    }
    if bang {
        command_map.push((Value::from("bang"), Value::Boolean(true)));
    }
    Ok(Value::Array(vec![
        Value::Map(command_map),
        Value::Map(vec![(Value::from("output"), Value::Boolean(true))]),
    ]))
}

fn substitute_arg(pattern: &str, replacement: &str, flags: &str) -> String {
    const DELIMITER: char = '\u{1}';
    format!(
        "{delimiter}{pattern}{delimiter}{replacement}{delimiter}{flags}",
        delimiter = DELIMITER,
        pattern = pattern.replace(DELIMITER, "\\\u{1}"),
        replacement = replacement.replace(DELIMITER, "\\\u{1}"),
    )
}

fn authorize_operation(operation: NvimOp, capability: Capability) -> Result<()> {
    if operation.capability() != capability {
        bail!("nvim operation has the wrong capability")
    }
    Ok(())
}

fn authorize() -> Result<()> {
    let operation = NvimOp::from_str("nvim_command")
        .map_err(|_| anyhow::anyhow!("nvim command operation is not registered"))?;
    authorize_operation(operation, Capability::Execute)
}

#[cfg(test)]
mod tests {
    use super::request;
    use crate::nvim_ui::stream::wire::ExCommand;
    use rmpv::Value;

    #[test]
    fn substitute_is_structured_and_captures_output() {
        let Value::Array(args) = request(
            &ExCommand::Substitute {
                pattern: "foo".into(),
                replacement: "bar".into(),
                global: true,
                ignore_case: false,
            },
            Some(1),
        )
        .expect("authorized command") else {
            panic!("command request must be an argument array")
        };
        let Value::Map(command) = &args[0] else {
            panic!("command must be a map")
        };
        assert!(command.iter().any(|(key, value)| {
            key.as_str() == Some("cmd") && value.as_str() == Some("substitute")
        }));
        assert!(args[1].as_map().is_some());
    }

    #[test]
    fn all_command_names_are_fixed() {
        let commands = [
            ExCommand::Write,
            ExCommand::WriteAll,
            ExCommand::Quit,
            ExCommand::ForceQuit,
            ExCommand::BufferDelete,
            ExCommand::NoHighlight,
            ExCommand::EditReload,
            ExCommand::Undo,
            ExCommand::Redo,
            ExCommand::Sort {
                reverse: false,
                numeric: false,
                unique: false,
                ignore_case: false,
            },
        ];
        for command in commands {
            assert!(request(&command, Some(1)).is_ok());
        }
    }
}
