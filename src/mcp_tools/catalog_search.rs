//! Definitions of bash, glob, grep and web_fetch.

use super::schema::{boolean, choice, integer, list_of, object, string, tool};
use serde_json::{json, Value};

pub fn bash() -> Value {
    tool(
        "bash",
        "Run a shell command and return its output and exit code. It runs in the project \
         folder (or cwd) with your shell, with no input attached, so a command that waits \
         for input will hang until it times out. Output over about 30 KB per stream is cut \
         in the middle and says so. On timeout the command and everything it started are \
         killed. A non-zero exit code is a normal result, not a tool error. For reading, \
         searching and editing files, prefer read_file, grep, glob and edit_file.",
        object(
            vec![
                ("command", string("The shell command line to run.")),
                ("cwd", string("Folder to run in. Default: the project folder.")),
                (
                    "timeout_ms",
                    integer("Kill the command after this many milliseconds. Default 120000, at most 600000."),
                ),
                ("description", string("A few words saying what the command does, for the user.")),
            ],
            &["command"],
        ),
        object(
            vec![
                ("exit_code", json!({ "type": ["integer", "null"], "description": "Exit code, or null if it was killed." })),
                ("signal", json!({ "type": ["integer", "null"], "description": "Signal number that ended it, if any." })),
                ("timed_out", boolean("True when the timeout killed the command.")),
                ("stdout", string("Standard output, cut in the middle if too long.")),
                ("stderr", string("Standard error, cut in the middle if too long.")),
                ("stdout_omitted_bytes", integer("Bytes cut from stdout.")),
                ("stderr_omitted_bytes", integer("Bytes cut from stderr.")),
                ("duration_ms", integer("How long it ran.")),
            ],
            &["exit_code", "timed_out", "stdout", "stderr", "stdout_omitted_bytes", "stderr_omitted_bytes", "duration_ms"],
        ),
    )
}

pub fn glob() -> Value {
    tool(
        "glob",
        "Find files by name. The pattern uses .gitignore glob syntax: '*.rs' matches at any \
         depth, 'src/**/*.ts' matches under src, '**/test_*.py' matches by name. Files \
         ignored by .gitignore and the .git folder are skipped. Returns paths relative to \
         the search folder, sorted by name, at most 1000.",
        object(
            vec![
                ("pattern", string("The glob pattern.")),
                (
                    "path",
                    string("Folder to search. Default: the project folder."),
                ),
            ],
            &["pattern"],
        ),
        object(
            vec![
                ("root", string("The folder searched.")),
                ("files", list_of(string("A file path."), "Matching files.")),
                (
                    "truncated",
                    boolean("True when more than 1000 files matched."),
                ),
            ],
            &["root", "files", "truncated"],
        ),
    )
}

pub fn grep() -> Value {
    let item = object(
        vec![
            ("path", string("File path relative to the search folder.")),
            ("line", integer("Line number (content mode).")),
            ("text", string("The line (content mode).")),
            ("count", integer("Matching lines in the file (count mode).")),
            (
                "context",
                boolean("True for a context line that does not match (content mode)."),
            ),
        ],
        &["path"],
    );
    tool(
        "grep",
        "Search file contents with a regular expression (Rust regex syntax). Respects \
         .gitignore and skips binary files. output_mode 'files' (default) lists files that \
         match; 'content' lists matching lines as path:line:text, with context lines as \
         path-line-text; 'count' gives the number of matching lines per file.",
        object(
            vec![
                ("pattern", string("Regular expression to find.")),
                ("path", string("File or folder to search. Default: the project folder.")),
                ("glob", string("Only search files matching this glob, such as '*.rs'.")),
                (
                    "case",
                    choice(
                        &["sensitive", "insensitive", "smart"],
                        "Default 'sensitive'. 'smart' ignores case unless the pattern has a capital letter.",
                    ),
                ),
                ("context", integer("Lines of context before and after each match (content mode). At most 20.")),
                ("output_mode", choice(&["files", "content", "count"], "What to return. Default 'files'.")),
                ("head_limit", integer("Stop after this many results. Default 250, at most 2000.")),
            ],
            &["pattern"],
        ),
        object(
            vec![
                ("mode", choice(&["files", "content", "count"], "The mode used.")),
                ("results", list_of(item, "Results, in file order.")),
                ("truncated", boolean("True when head_limit stopped the search.")),
            ],
            &["mode", "results", "truncated"],
        ),
    )
}

pub fn web_fetch() -> Value {
    tool(
        "web_fetch",
        "Download one http or https page and return it as readable text. HTML becomes \
         Markdown-like text (headings, links, lists, code); other text types are returned \
         as is. Images, PDFs and other binary types are refused. 30 second timeout. Use it \
         for documentation and articles; it cannot run scripts or log in.",
        object(
            vec![
                ("url", string("The http or https address.")),
                (
                    "max_bytes",
                    integer(
                        "Stop downloading after this many bytes. Default 200000, at most 2000000.",
                    ),
                ),
            ],
            &["url"],
        ),
        object(
            vec![
                ("url", string("The final address, after redirects.")),
                ("status", integer("HTTP status code.")),
                ("content_type", string("The Content-Type header.")),
                (
                    "truncated",
                    boolean("True when max_bytes cut the download."),
                ),
                ("content", string("The page as readable text.")),
            ],
            &["url", "status", "content_type", "truncated", "content"],
        ),
    )
}
