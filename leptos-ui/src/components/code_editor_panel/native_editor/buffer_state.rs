//! Editor buffer state — wraps floem-editor-core Buffer + Cursor for use in Leptos.

use std::cell::RefCell;
use std::rc::Rc;

use floem_editor_core::buffer::rope_text::RopeText;
use floem_editor_core::buffer::Buffer;
use floem_editor_core::command::EditCommand;
use floem_editor_core::cursor::{Cursor, CursorMode};
use floem_editor_core::editor::{Action, EditConf};
use floem_editor_core::register::Register;
use floem_editor_core::selection::Selection;

/// A clipboard implementation for the WASM environment.
pub struct WasmClipboard;

impl floem_editor_core::register::Clipboard for WasmClipboard {
    fn get_string(&mut self) -> Option<String> {
        None // Clipboard reads are async in browsers; handled separately
    }

    fn put_string(&mut self, _s: impl AsRef<str>) {
        // Clipboard writes are async in browsers; handled separately
    }
}

/// Shared editor state wrapping floem-editor-core primitives.
#[derive(Clone)]
pub struct EditorBuffer {
    inner: Rc<RefCell<EditorBufferInner>>,
}

struct EditorBufferInner {
    buffer: Buffer,
    cursor: Cursor,
    register: Register,
}

impl EditorBuffer {
    /// Create a new editor buffer with the given content.
    pub fn new(content: &str) -> Self {
        let buffer = Buffer::new(content);
        let cursor = Cursor::new(CursorMode::Insert(Selection::caret(0)), None, None);
        let register = Register::default();
        Self {
            inner: Rc::new(RefCell::new(EditorBufferInner {
                buffer,
                cursor,
                register,
            })),
        }
    }

    /// Replace buffer content entirely (e.g. on file switch).
    pub fn set_content(&self, content: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.buffer = Buffer::new(content);
        inner.cursor = Cursor::new(CursorMode::Insert(Selection::caret(0)), None, None);
    }

    /// Get the full text content as a String.
    pub fn content(&self) -> String {
        let inner = self.inner.borrow();
        inner.buffer.text().to_string()
    }

    /// Number of lines in the buffer.
    pub fn num_lines(&self) -> usize {
        let inner = self.inner.borrow();
        inner.buffer.num_lines()
    }

    /// Get content of a single line (0-indexed). Returns empty if out of range.
    pub fn line_content(&self, line: usize) -> String {
        let inner = self.inner.borrow();
        if line >= inner.buffer.num_lines() {
            return String::new();
        }
        inner.buffer.line_content(line).to_string()
    }

    /// Current cursor position as (line, col), both 0-indexed.
    pub fn cursor_pos(&self) -> (usize, usize) {
        let inner = self.inner.borrow();
        inner.buffer.offset_to_line_col(inner.cursor.offset())
    }

    /// Current cursor byte offset.
    pub fn cursor_offset(&self) -> usize {
        self.inner.borrow().cursor.offset()
    }

    /// Set cursor to a specific line and column (0-indexed).
    pub fn set_cursor_pos(&self, line: usize, col: usize) {
        let mut inner = self.inner.borrow_mut();
        let offset = inner.buffer.offset_of_line_col(line, col);
        inner.cursor = Cursor::new(CursorMode::Insert(Selection::caret(offset)), None, None);
    }

    /// Insert a string at the current cursor position.
    pub fn insert(&self, s: &str) {
        let mut inner = self.inner.borrow_mut();
        let EditorBufferInner { buffer, cursor, .. } = &mut *inner;
        let _deltas = Action::insert(cursor, buffer, s, &no_unmatched_pair, true, false);
    }

    /// Execute an edit command (delete, newline, undo, redo, etc.).
    pub fn do_edit(&self, cmd: &EditCommand) {
        let mut inner = self.inner.borrow_mut();
        let EditorBufferInner {
            buffer,
            cursor,
            register,
        } = &mut *inner;
        let conf = EditConf {
            comment_token: "//",
            modal: false,
            smart_tab: true,
            keep_indent: true,
            auto_indent: true,
        };
        let mut clipboard = WasmClipboard;
        let _deltas = Action::do_edit(cursor, buffer, cmd, &mut clipboard, register, conf);
    }

    /// Whether the buffer has been modified from its pristine state.
    pub fn is_modified(&self) -> bool {
        !self.inner.borrow().buffer.is_pristine()
    }

    /// Mark the buffer as pristine (saved).
    pub fn set_pristine(&self) {
        self.inner.borrow_mut().buffer.set_pristine();
    }

    /// Total byte length.
    pub fn len(&self) -> usize {
        self.inner.borrow().buffer.len()
    }

    /// Jump cursor to a specific line (0-indexed), placing at first non-blank.
    pub fn jump_to_line(&self, line: usize) {
        let mut inner = self.inner.borrow_mut();
        let target_line = line.min(inner.buffer.last_line());
        let offset = inner.buffer.offset_of_line(target_line);
        inner.cursor = Cursor::new(CursorMode::Insert(Selection::caret(offset)), None, None);
    }
}

/// Stub for `prev_unmatched` parameter — no auto-indent matching for now.
fn no_unmatched_pair(_buffer: &Buffer, _c: char, _offset: usize) -> Option<usize> {
    None
}
