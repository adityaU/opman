//! Decode the Neovim notifications the edit engine paints from.
//!
//! Everything here comes out of Neovim rather than out of opman guessing. The
//! command line, messages and tab line arrive as `redraw` UI events because the
//! session attaches with `ext_cmdline`, `ext_messages` and `ext_tabline`; opman
//! neither parses keystrokes nor decides when a line is finished.
//!
//! Redraw events are batched: `["msg_show", [args…], [args…]]` is one event
//! carrying two occurrences. A few Neovim versions still send the unbatched
//! `["msg_show", arg, arg]`, so both shapes are accepted.

use rmpv::Value;

use crate::nvim_ui::NvimNotification;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Notification {
    Lines {
        buffer: u64,
        changedtick: u64,
        first_line: usize,
        last_line: usize,
        new_last_line: usize,
        lines: Vec<String>,
    },
    Detached {
        buffer: u64,
    },
    Reloaded {
        buffer: u64,
        changedtick: Option<u64>,
    },
    Message {
        kind: String,
        text: String,
    },
    CmdlineShow {
        content: String,
        position: u32,
        first_char: String,
    },
    CmdlinePos {
        position: u32,
    },
    CmdlineHide,
    /// Neovim finished a batch of UI events; its state is now consistent.
    Flush,
    /// A command-mode keymap fired inside Neovim and asked opman to act.
    Action {
        name: String,
    },
}

pub(crate) fn decode(notification: &NvimNotification) -> Vec<Notification> {
    let Some(value) = decode_value(&notification.params) else {
        return Vec::new();
    };
    match notification.method.as_str() {
        "nvim_buf_lines_event" => parse_lines(&value).into_iter().collect(),
        "nvim_buf_detach_event" => parse_buffer_only(&value)
            .map(|buffer| Notification::Detached { buffer })
            .into_iter()
            .collect(),
        "nvim_buf_reload_event" => parse_reload(&value).into_iter().collect(),
        "redraw" => parse_redraw(&value),
        "opman_action" => parse_action(&value).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn decode_value(bytes: &[u8]) -> Option<Value> {
    let mut input = bytes;
    let value = rmpv::decode::read_value(&mut input).ok()?;
    input.is_empty().then_some(value)
}

fn parse_action(value: &Value) -> Option<Notification> {
    Some(Notification::Action {
        name: value.as_array()?.first()?.as_str()?.to_owned(),
    })
}

fn parse_lines(value: &Value) -> Option<Notification> {
    let fields = value.as_array()?;
    let buffer = positive(fields.first()?)?;
    let changedtick = positive(fields.get(1)?)?;
    let first_line = nonnegative(fields.get(2)?)?;
    let last_line = signed(fields.get(3)?)?;
    let (new_last_line, lines_value) = match fields.get(4)?.as_array() {
        Some(lines) => (first_line.checked_add(lines.len())?, lines),
        None => (nonnegative(fields.get(4)?)?, fields.get(5)?.as_array()?),
    };
    let last_line = if last_line < 0 {
        first_line
    } else {
        usize::try_from(last_line).ok()?
    };
    let lines = lines_value
        .iter()
        .map(|line| line.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    Some(Notification::Lines {
        buffer,
        changedtick,
        first_line,
        last_line,
        new_last_line,
        lines,
    })
}

fn parse_buffer_only(value: &Value) -> Option<u64> {
    positive(value.as_array()?.first()?)
}

fn parse_reload(value: &Value) -> Option<Notification> {
    let fields = value.as_array()?;
    Some(Notification::Reloaded {
        buffer: positive(fields.first()?)?,
        changedtick: fields.get(1).and_then(positive),
    })
}

fn parse_redraw(value: &Value) -> Vec<Notification> {
    value
        .as_array()
        .into_iter()
        .flat_map(|events| events.iter())
        .filter_map(Value::as_array)
        .flat_map(|event| redraw_event(event))
        .collect()
}

/// One redraw event, expanded over however many occurrences it batches.
fn redraw_event(event: &[Value]) -> Vec<Notification> {
    let Some(name) = event.first().and_then(Value::as_str) else {
        return Vec::new();
    };
    if name == "flush" {
        return vec![Notification::Flush];
    }
    if name == "cmdline_hide" {
        return vec![Notification::CmdlineHide];
    }
    let rest = &event[1..];
    // Batched events wrap each occurrence's arguments in their own array.
    let batches: Vec<&[Value]> = match rest.first().and_then(Value::as_array) {
        Some(_) => rest
            .iter()
            .filter_map(Value::as_array)
            .map(Vec::as_slice)
            .collect(),
        None => vec![rest],
    };
    batches
        .into_iter()
        .filter_map(|args| occurrence(name, args))
        .collect()
}

fn occurrence(name: &str, args: &[Value]) -> Option<Notification> {
    match name {
        "msg_show" => Some(Notification::Message {
            kind: args.first()?.as_str()?.to_owned(),
            text: chunk_text(args.get(1)?),
        }),
        "msg_history_show" => Some(Notification::Message {
            kind: "history".into(),
            text: history_text(args.first()?),
        }),
        "cmdline_show" => Some(Notification::CmdlineShow {
            content: chunk_text(args.first()?),
            position: args.get(1).and_then(Value::as_u64).unwrap_or(0) as u32,
            first_char: args.get(2).and_then(Value::as_str).unwrap_or("").to_owned(),
        }),
        "cmdline_pos" => Some(Notification::CmdlinePos {
            position: args.first().and_then(Value::as_u64).unwrap_or(0) as u32,
        }),
        _ => None,
    }
}

/// Flatten `[[attrs, text], …]` — the shape both messages and the command line
/// use for styled runs — into the plain text opman renders.
fn chunk_text(value: &Value) -> String {
    value
        .as_array()
        .into_iter()
        .flat_map(|chunks| chunks.iter())
        .filter_map(|chunk| {
            let fields = chunk.as_array()?;
            fields
                .iter()
                .find_map(Value::as_str)
                .map(std::borrow::ToOwned::to_owned)
        })
        .collect()
}

/// `msg_history_show` nests one level deeper: `[[kind, chunks], …]`.
fn history_text(value: &Value) -> String {
    value
        .as_array()
        .into_iter()
        .flat_map(|entries| entries.iter())
        .filter_map(|entry| Some(chunk_text(entry.as_array()?.get(1)?)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn positive(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|n| u64::try_from(n).ok()))
        .or_else(|| {
            crate::nvim_ui::rpc::value::ext_or_int(value)
                .ok()
                .and_then(|n| u64::try_from(n).ok())
        })
}

fn nonnegative(value: &Value) -> Option<usize> {
    positive(value).and_then(|n| usize::try_from(n).ok())
}

fn signed(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
}

#[cfg(test)]
#[path = "notifications_tests.rs"]
mod notifications_tests;
