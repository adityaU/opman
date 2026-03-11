use similar::{ChangeTag, TextDiff};

/// Read the full terminal buffer (scrollback + visible screen) from a vt100 parser.
///
/// The vt100 `Screen` only exposes a viewport of `screen_rows` lines at a time.
/// To read the full history we temporarily shift the scrollback offset, read each
/// window of rows, then restore the original offset.
///
/// If `last_n` is `Some(n)`, only the last `n` lines of the full buffer are returned.
/// Otherwise the entire buffer is returned.
pub fn read_full_terminal_buffer(parser: &mut vt100::Parser, last_n: Option<usize>) -> String {
    let original_offset = parser.screen().scrollback();
    let (screen_rows, cols) = parser.screen().size();
    let screen_rows = screen_rows as usize;

    // Discover how many scrollback rows exist by setting offset to max.
    parser.set_scrollback(usize::MAX);
    let total_scrollback = parser.screen().scrollback();

    let total_lines = total_scrollback + screen_rows;

    // Determine which lines to collect.
    let (skip_lines, lines_wanted) = if let Some(n) = last_n {
        let n = n.min(total_lines);
        (total_lines - n, n)
    } else {
        (0, total_lines)
    };

    let mut lines: Vec<String> = Vec::with_capacity(lines_wanted);

    // Walk from top of buffer (max scrollback) downward.
    let mut sb = total_scrollback;
    loop {
        parser.set_scrollback(sb);
        let window_start = total_scrollback - sb;

        for (i, row_text) in parser.screen().rows(0, cols).enumerate() {
            let abs_line = window_start + i;
            if abs_line < skip_lines {
                continue;
            }
            if lines.len() >= lines_wanted {
                break;
            }
            lines.push(row_text);
        }

        if lines.len() >= lines_wanted {
            break;
        }

        if sb >= screen_rows {
            sb -= screen_rows;
        } else {
            if sb == 0 {
                break;
            }
            sb = 0;
        }
    }

    // Restore original scrollback position so the UI is unaffected.
    parser.set_scrollback(original_offset);

    // Trim trailing empty lines.
    while lines.last().map_or(false, |l| l.trim().is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

/// Diff two file snapshots line-by-line using the `similar` crate and return
/// (added_lines, deleted_lines) as 1-based line numbers in the *new* file.
///
/// - `added` contains line numbers that are new or changed in the new file.
/// - `deleted` contains line numbers in the new file *after which* old lines were removed.
///   (If deletions occur at the very start, line 1 is used.)
pub fn diff_snapshot_lines(old: &str, new: &str) -> (Vec<usize>, Vec<usize>) {
    let diff = TextDiff::from_lines(old, new);
    let mut added = Vec::new();
    let mut deleted = Vec::new();

    let mut new_line: usize = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                new_line += 1;
            }
            ChangeTag::Insert => {
                new_line += 1;
                added.push(new_line);
            }
            ChangeTag::Delete => {
                deleted.push(new_line.max(1));
            }
        }
    }

    deleted.dedup();

    (added, deleted)
}

/// Parse `git diff --unified=0` output and return (added_lines, deleted_lines).
///
/// Hunk headers look like `@@ -old_start,old_count +new_start,new_count @@`.
#[cfg(test)]
pub(crate) fn parse_unified_diff(diff: &str) -> (Vec<usize>, Vec<usize>) {
    let mut added = Vec::new();
    let mut deleted = Vec::new();

    for line in diff.lines() {
        if !line.starts_with("@@ ") {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        let old_part = parts[1].trim_start_matches('-');
        let old_count = if let Some((_start, count)) = old_part.split_once(',') {
            count.parse::<usize>().unwrap_or(0)
        } else {
            1
        };

        let new_part = parts[2].trim_start_matches('+');
        let (new_start, new_count) = if let Some((start, count)) = new_part.split_once(',') {
            (
                start.parse::<usize>().unwrap_or(1),
                count.parse::<usize>().unwrap_or(0),
            )
        } else {
            (new_part.parse::<usize>().unwrap_or(1), 1)
        };

        for i in 0..new_count {
            added.push(new_start + i);
        }
        if old_count > 0 && new_count == 0 {
            deleted.push(new_start.max(1));
        }
    }

    (added, deleted)
}
