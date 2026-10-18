/// Map file extension to MIME type for binary file serving.
pub(crate) fn mime_from_extension(path: &str) -> String {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        // Images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        // 3D models
        "stl" => "model/stl",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "obj" => "model/obj",
        // Audio
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "m4a" => "audio/mp4",
        "weba" => "audio/webm",
        // Video
        "mp4" => "video/mp4",
        "m4v" => "video/mp4",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        // Documents
        "pdf" => "application/pdf",
        "csv" => "text/csv",
        "xlsx" | "xls" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" | "ppt" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        "docx" | "doc" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        // Fallback
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Detect language from file extension for CodeMirror syntax highlighting.
pub(crate) fn detect_language(path: &str) -> String {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "py" | "pyw" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cxx" | "cc" | "hpp" | "hxx" => "cpp",
        "json" => "json",
        "html" | "htm" => "html",
        "css" | "scss" | "less" => "css",
        "md" | "mdx" | "markdown" => "markdown",
        "sql" => "sql",
        "xml" | "svg" => "xml",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sh" | "bash" | "zsh" => "shell",
        "fish" => "shell",
        "lua" => "lua",
        "rb" => "ruby",
        "php" => "php",
        "vue" => "vue",
        "svelte" => "svelte",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "mmd" | "mermaid" => "mermaid",
        "ini" | "cfg" | "conf" => "ini",
        "proto" => "protobuf",
        "graphql" | "gql" => "graphql",
        "diff" | "patch" => "diff",
        "dockerfile" => "dockerfile",
        "makefile" => "makefile",
        _ => "text",
    }
    .to_string()
}
