//! Rust-native code editor built on floem-editor-core + syntect.
//! Replaces the CodeMirror JS bridge with a fully WASM-native editor.

pub mod buffer_state;
pub mod helpers;
pub mod highlighter;
pub mod input;
pub mod renderer;

pub use renderer::NativeEditor;
