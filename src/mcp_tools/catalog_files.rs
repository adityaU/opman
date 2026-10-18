//! Definitions of the file tools: names, model-facing descriptions, input and output schemas.

use super::schema::{boolean, integer, list_of, object, string, tool};
use serde_json::{json, Value};

pub fn read_file() -> Value {
    tool(
        "read_file",
        "Read a file. Returns text lines with line numbers (number, tab, text). Reads up \
         to 2000 lines by default; for longer files pass offset (first line, counting from 1) \
         and limit to read in pieces. Lines over 2000 characters are cut. Binary files return \
         metadata so the user can open a preview. Relative paths start at the project folder.",
        object(
            vec![
                (
                    "path",
                    string("File to read. Relative to the project, or absolute."),
                ),
                (
                    "offset",
                    integer("First line to return, counting from 1. Default 1."),
                ),
                (
                    "limit",
                    integer("Number of lines to return. Default 2000, at most 10000."),
                ),
            ],
            &["path"],
        ),
        object(
            vec![
                ("path", string("The file, as shown to the user.")),
                (
                    "start_line",
                    integer("First line returned (0 for an empty file)."),
                ),
                ("end_line", integer("Last line returned.")),
                (
                    "content",
                    string("The returned lines without line numbers."),
                ),
                (
                    "has_more",
                    boolean("True when the file has more lines after end_line."),
                ),
                (
                    "long_lines_clipped",
                    boolean("True when a very long line was cut."),
                ),
                (
                    "binary",
                    boolean("True when the file contains binary data instead of text."),
                ),
                ("bytes", integer("File size in bytes for binary files.")),
            ],
            &[
                "path",
                "start_line",
                "end_line",
                "content",
                "has_more",
                "long_lines_clipped",
            ],
        ),
    )
}

pub fn write_file() -> Value {
    tool(
        "write_file",
        "Create a file, or replace it completely, with the given content. Missing parent \
         folders are created. The write is atomic. To change part of an existing file, use \
         edit_file instead.",
        object(
            vec![
                (
                    "path",
                    string("File to write. Relative to the project, or absolute."),
                ),
                ("content", string("The full new content of the file.")),
            ],
            &["path", "content"],
        ),
        object(
            vec![
                ("path", string("The file, as shown to the user.")),
                ("bytes", integer("Bytes written.")),
                ("created", boolean("True if the file did not exist before.")),
            ],
            &["path", "bytes", "created"],
        ),
    )
}

pub fn edit_file() -> Value {
    tool(
        "edit_file",
        "Replace exact text in an existing file. old_string must match the file exactly, \
         including spaces, tabs and line breaks, and must appear once; if it appears more \
         than once, add surrounding lines to make it unique, or set replace_all to change \
         every occurrence. Read the file first so you can copy the text. Returns a short diff.",
        object(
            vec![
                (
                    "path",
                    string("File to edit. Relative to the project, or absolute."),
                ),
                (
                    "old_string",
                    string("The exact text to replace. Must not be empty."),
                ),
                (
                    "new_string",
                    string("The text to put in its place. Must differ from old_string."),
                ),
                (
                    "replace_all",
                    boolean("Replace every occurrence. Default false."),
                ),
            ],
            &["path", "old_string", "new_string"],
        ),
        object(
            vec![
                ("path", string("The file, as shown to the user.")),
                ("replacements", integer("How many places were changed.")),
                (
                    "first_line",
                    integer("Line number of the first change, counting from 1."),
                ),
                ("diff", string("The changed lines: '-' removed, '+' added.")),
            ],
            &["path", "replacements", "first_line", "diff"],
        ),
    )
}

pub fn list_dir() -> Value {
    let entry = object(
        vec![
            ("name", string("File or folder name.")),
            (
                "kind",
                json!({ "type": "string", "enum": ["dir", "file", "symlink", "other"] }),
            ),
            ("size", integer("Size in bytes, for files only.")),
        ],
        &["name", "kind"],
    );
    tool(
        "list_dir",
        "List the files and folders directly inside one folder (not recursive). Folders come \
         first and end with '/'. Hidden files are included. Use glob to search deeper.",
        object(
            vec![(
                "path",
                string("Folder to list. Default: the project folder."),
            )],
            &[],
        ),
        object(
            vec![
                ("path", string("The folder, as shown to the user.")),
                (
                    "entries",
                    list_of(entry, "The entries, folders first, then by name."),
                ),
                (
                    "truncated",
                    boolean("True when the folder has more than 1000 entries."),
                ),
            ],
            &["path", "entries", "truncated"],
        ),
    )
}
