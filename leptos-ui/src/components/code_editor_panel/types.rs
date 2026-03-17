//! Types, classification helpers, and small utilities for the code editor panel.

use crate::types::api::FileEntry;

// ── File classification (matches React api/files.ts classifyFile) ──

#[derive(Clone, Debug, PartialEq)]
pub enum FileRenderType {
    Code,
    Image,
    Audio,
    Video,
    Markdown,
    Html,
    Mermaid,
    Svg,
    Csv,
    Pdf,
    Binary,
}

/// Classify file by extension. Order matches React `classifyFile()` exactly.
pub fn classify_file(path: &str) -> FileRenderType {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" | "bmp" | "avif" => {
            FileRenderType::Image
        }
        "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" | "weba" => FileRenderType::Audio,
        "mp4" | "webm" | "ogv" | "mov" | "avi" | "mkv" => FileRenderType::Video,
        "pdf" => FileRenderType::Pdf,
        "csv" => FileRenderType::Csv,
        "md" | "mdx" | "markdown" => FileRenderType::Markdown,
        "html" | "htm" => FileRenderType::Html,
        "mmd" | "mermaid" => FileRenderType::Mermaid,
        "xlsx" | "xls" | "pptx" | "ppt" | "docx" | "doc" | "zip" | "tar" | "gz" | "rar" | "7z"
        | "exe" | "dll" | "so" | "dylib" | "wasm" | "bin" => FileRenderType::Binary,
        _ => FileRenderType::Code,
    }
}

/// Returns true for previewable types that support Code/Rendered toggle.
pub fn is_previewable_render_type(rt: &FileRenderType) -> bool {
    matches!(
        rt,
        FileRenderType::Markdown
            | FileRenderType::Html
            | FileRenderType::Mermaid
            | FileRenderType::Svg
    )
}

/// Returns true for binary-like types that skip content fetch.
pub fn is_binary_render_type(rt: &FileRenderType) -> bool {
    matches!(
        rt,
        FileRenderType::Image
            | FileRenderType::Audio
            | FileRenderType::Video
            | FileRenderType::Pdf
            | FileRenderType::Binary
    )
}

pub fn file_icon(entry: &FileEntry) -> &'static str {
    if entry.is_dir {
        return "D";
    }
    let lower = entry.name.to_lowercase();
    if lower.ends_with(".rs") {
        "Rs"
    } else if lower.ends_with(".ts") || lower.ends_with(".tsx") {
        "Ts"
    } else if lower.ends_with(".js") || lower.ends_with(".jsx") {
        "Js"
    } else if lower.ends_with(".py") {
        "Py"
    } else if lower.ends_with(".go") {
        "Go"
    } else if lower.ends_with(".md") {
        "Md"
    } else if lower.ends_with(".json") {
        "Jn"
    } else if lower.ends_with(".toml") {
        "Tm"
    } else if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        "Ym"
    } else if lower.ends_with(".css") {
        "Cs"
    } else if lower.ends_with(".html") {
        "Ht"
    } else {
        "F"
    }
}

pub fn format_size(size: u64) -> String {
    if size < 1024 {
        return format!("{} B", size);
    }
    if size < 1024 * 1024 {
        return format!("{:.1} KB", size as f64 / 1024.0);
    }
    format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
}

// ── CSV parsing (matches React CsvViewer hand-rolled parser) ───────

pub fn parse_csv(content: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut cells: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            if ch == '"' {
                if in_quotes && i + 1 < chars.len() && chars[i + 1] == '"' {
                    current.push('"');
                    i += 1;
                } else {
                    in_quotes = !in_quotes;
                }
            } else if ch == ',' && !in_quotes {
                cells.push(current.trim().to_string());
                current = String::new();
            } else {
                current.push(ch);
            }
            i += 1;
        }
        cells.push(current.trim().to_string());
        rows.push(cells);
    }
    rows
}

// ── Open file tab model ────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct OpenFile {
    pub path: String,
    pub name: String,
    pub language: String,
    pub content: String,
    /// Edited content (if different from saved content, like React's editedContent)
    pub edited_content: Option<String>,
    pub render_type: FileRenderType,
}

impl OpenFile {
    /// Returns the current display content (edited or original).
    pub fn current_content(&self) -> &str {
        self.edited_content.as_deref().unwrap_or(&self.content)
    }

    /// Whether the file has unsaved changes.
    pub fn is_modified(&self) -> bool {
        self.edited_content.is_some()
    }
}

// ── Delete confirmation model ──────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ConfirmDeleteEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

// ── View mode (Code / Rendered) ────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum EditorViewMode {
    Code,
    Rendered,
}

// ── Language → file extension mapping ──────────────────────────────

/// Map backend language names (e.g. "rust", "typescript") to syntect file extensions.
pub fn language_to_extension(language: &str) -> String {
    match language.to_lowercase().as_str() {
        "rust" => "rs",
        "typescript" | "typescriptreact" => "ts",
        "javascript" | "javascriptreact" => "js",
        "python" => "py",
        "go" | "golang" => "go",
        "c" => "c",
        "cpp" | "c++" => "cpp",
        "java" => "java",
        "ruby" => "rb",
        "html" => "html",
        "css" => "css",
        "scss" => "scss",
        "json" | "jsonc" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "shell" | "bash" | "sh" | "zsh" => "sh",
        "sql" => "sql",
        "markdown" | "md" => "md",
        "lua" => "lua",
        "php" => "php",
        "swift" => "swift",
        "kotlin" => "kt",
        "scala" => "scala",
        "haskell" => "hs",
        "elixir" => "ex",
        "erlang" => "erl",
        "clojure" => "clj",
        "r" => "r",
        "perl" => "pl",
        "dart" => "dart",
        "zig" => "zig",
        "objective-c" | "objc" => "m",
        "objective-cpp" | "objcpp" => "mm",
        "groovy" => "groovy",
        "d" => "d",
        "pascal" | "delphi" => "pas",
        "ocaml" => "ml",
        "lisp" | "scheme" => "lisp",
        "latex" | "tex" => "tex",
        "powershell" => "ps1",
        "vim" | "viml" => "vim",
        "diff" | "patch" => "diff",
        "dockerfile" => "Dockerfile",
        "makefile" => "Makefile",
        other => other,
    }
    .to_string()
}
