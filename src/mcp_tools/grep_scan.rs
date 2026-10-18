//! Search one file for a regex, line by line, in bounded memory.

use regex::Regex;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

/// Longest slice of a line that is searched. The rest of a longer line is skipped.
const LINE_BYTES: u64 = 64 * 1024;
/// Longest line text kept in a result.
const SHOWN_CHARS: usize = 500;

/// What the caller needs from a file, so cheap modes stop early.
#[derive(Clone, Copy)]
pub enum Want {
    /// Only whether anything matches.
    Any,
    /// How many lines match.
    Count,
    /// Matching lines with this many lines of context around them.
    Lines { context: usize },
}

pub struct Hit {
    pub line: u64,
    pub text: String,
    pub is_match: bool,
}

#[derive(Default)]
pub struct FileScan {
    pub matches: u64,
    pub hits: Vec<Hit>,
}

/// `Ok(None)` for a binary file. `room` is how many hits may still be collected.
pub fn scan_file(
    path: &Path,
    regex: &Regex,
    want: Want,
    room: usize,
) -> io::Result<Option<FileScan>> {
    let mut reader = BufReader::new(File::open(path)?);
    if reader.fill_buf()?.contains(&0) {
        return Ok(None);
    }
    let context = match want {
        Want::Lines { context } => context,
        _ => 0,
    };
    let mut scan = FileScan::default();
    let mut before: VecDeque<Hit> = VecDeque::with_capacity(context);
    let mut after_left = 0usize;
    let mut buf = Vec::new();
    let mut line_no = 0u64;
    loop {
        buf.clear();
        let read = (&mut reader).take(LINE_BYTES).read_until(b'\n', &mut buf)?;
        if read == 0 {
            break;
        }
        if !buf.ends_with(b"\n") && read as u64 == LINE_BYTES {
            reader.skip_until(b'\n')?;
        }
        line_no += 1;
        let line = String::from_utf8_lossy(&buf);
        let line = line.trim_end_matches(['\n', '\r']);
        if !regex.is_match(line) {
            if after_left > 0 && scan.hits.len() < room {
                after_left -= 1;
                scan.hits.push(hit(line_no, line, false));
            } else if context > 0 {
                if before.len() == context {
                    before.pop_front();
                }
                before.push_back(hit(line_no, line, false));
            }
            continue;
        }
        scan.matches += 1;
        match want {
            Want::Any => return Ok(Some(scan)),
            Want::Count => {}
            Want::Lines { .. } => {
                if scan.hits.len() >= room {
                    return Ok(Some(scan));
                }
                scan.hits.extend(before.drain(..));
                scan.hits.push(hit(line_no, line, true));
                after_left = context;
            }
        }
    }
    Ok(Some(scan))
}

fn hit(line: u64, text: &str, is_match: bool) -> Hit {
    let text = match text.char_indices().nth(SHOWN_CHARS) {
        Some((cut, _)) => format!("{}...", &text[..cut]),
        None => text.to_string(),
    };
    Hit {
        line,
        text,
        is_match,
    }
}
