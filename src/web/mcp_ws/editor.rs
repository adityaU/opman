//! Editor tool handlers (`web_editor_*`).

use crate::web::types::{ServerState, WebEvent};

pub(crate) async fn handle_editor_open(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'path' argument")?;
    let line = args.get("line").and_then(|v| v.as_u64()).map(|v| v as u32);

    // Resolve path relative to project root if not absolute
    let resolved_path = if std::path::Path::new(path).is_absolute() {
        path.to_string()
    } else {
        match state.web_state.get_working_dir().await {
            Some(dir) => dir.join(path).to_string_lossy().to_string(),
            None => path.to_string(),
        }
    };

    // Verify file exists
    if !std::path::Path::new(&resolved_path).exists() {
        return Err(format!("File not found: {}", resolved_path));
    }

    // Emit SSE event to tell frontend to open the file
    let _ = state.event_tx.send(WebEvent::McpEditorOpen {
        path: resolved_path.clone(),
        line,
    });

    let msg = match line {
        Some(l) => format!("Opened '{}' at line {}", resolved_path, l),
        None => format!("Opened '{}'", resolved_path),
    };
    Ok(msg)
}

pub(crate) async fn handle_editor_read(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'path' argument")?;
    let start_line = args.get("start_line").and_then(|v| v.as_u64());
    let end_line = args.get("end_line").and_then(|v| v.as_u64());

    // Resolve path
    let resolved_path = if std::path::Path::new(path).is_absolute() {
        std::path::PathBuf::from(path)
    } else {
        match state.web_state.get_working_dir().await {
            Some(dir) => dir.join(path),
            None => std::path::PathBuf::from(path),
        }
    };

    // Read the file
    let content = tokio::fs::read_to_string(&resolved_path)
        .await
        .map_err(|e| format!("Failed to read '{}': {}", resolved_path.display(), e))?;

    // Apply line range if specified
    match (start_line, end_line) {
        (Some(s), Some(e)) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = (s as usize).saturating_sub(1); // 1-based to 0-based
            let end = std::cmp::min(e as usize, lines.len());
            if start >= lines.len() {
                return Err(format!(
                    "start_line {} is past end of file ({} lines)",
                    s,
                    lines.len()
                ));
            }
            Ok(lines[start..end].join("\n"))
        }
        (Some(s), None) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = (s as usize).saturating_sub(1);
            if start >= lines.len() {
                return Err(format!(
                    "start_line {} is past end of file ({} lines)",
                    s,
                    lines.len()
                ));
            }
            Ok(lines[start..].join("\n"))
        }
        (None, Some(e)) => {
            let lines: Vec<&str> = content.lines().collect();
            let end = std::cmp::min(e as usize, lines.len());
            Ok(lines[..end].join("\n"))
        }
        (None, None) => Ok(content),
    }
}

pub(crate) async fn handle_editor_list(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let subpath = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let max_depth = args
        .get("depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(3) as usize;

    let base_dir = match state.web_state.get_working_dir().await {
        Some(dir) => {
            if subpath.is_empty() {
                dir
            } else {
                dir.join(subpath)
            }
        }
        None => return Err("No active project directory".to_string()),
    };

    if !base_dir.exists() {
        return Err(format!("Directory not found: {}", base_dir.display()));
    }

    // Walk the directory tree up to max_depth
    let mut files = Vec::new();
    collect_files(&base_dir, &base_dir, 0, max_depth, &mut files);

    let result = serde_json::json!({
        "root": base_dir.to_string_lossy(),
        "files": files,
        "count": files.len()
    });
    Ok(result.to_string())
}

/// Recursively collect file paths, skipping common ignore patterns.
fn collect_files(
    root: &std::path::Path,
    dir: &std::path::Path,
    depth: usize,
    max_depth: usize,
    files: &mut Vec<String>,
) {
    if depth > max_depth {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files and common ignore directories
        if name.starts_with('.')
            || name == "node_modules"
            || name == "target"
            || name == "__pycache__"
            || name == "dist"
            || name == "build"
        {
            continue;
        }

        if path.is_dir() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            files.push(format!("{}/", rel.to_string_lossy()));
            collect_files(root, &path, depth + 1, max_depth, files);
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            files.push(rel.to_string_lossy().to_string());
        }
    }
}
