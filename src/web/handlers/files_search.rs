//! File browsing, reading, writing, and helper utilities.

use axum::extract::State;
use axum::response::{IntoResponse, Json};

use super::super::super::auth::AuthUser;
use super::super::super::error::{WebError, WebResult};
use super::super::super::types::*;
use super::super::common::resolve_project_dir;

/// GET /api/files/search?q=...&limit=... — fuzzy file search across the project tree.
///
/// Walks the project directory recursively (respecting .gitignore and skipping hidden dirs)
/// and returns files/dirs whose path contains all query segments (split by space).
/// Results are scored by path length (shorter = more relevant) and capped at `limit`.
pub async fn search_files(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<FileSearchQuery>,
) -> WebResult<impl IntoResponse> {
    let q = query.q.trim().to_string();
    if q.is_empty() {
        return Ok(Json(FileSearchResponse {
            query: q,
            entries: Vec::new(),
        }));
    }

    let dir = resolve_project_dir(&state).await?;
    let limit = query.limit.min(50);

    // Run the blocking recursive walk on a spawn_blocking thread
    let results = tokio::task::spawn_blocking(move || search_files_sync(&dir, &q, limit))
        .await
        .map_err(|e| WebError::Internal(format!("Search task failed: {e}")))?;

    Ok(Json(FileSearchResponse {
        query: query.q,
        entries: results,
    }))
}

/// Synchronous recursive file search with fuzzy matching.
pub(super) fn search_files_sync(root: &str, query: &str, limit: usize) -> Vec<FileSearchEntry> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    use std::fs;
    use std::path::Path;

    let root_path = Path::new(root);

    // Parse query into lowercase segments for multi-term matching
    let terms: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    if terms.is_empty() {
        return Vec::new();
    }

    // Load .gitignore patterns (simple top-level only)
    let gitignore_patterns = load_gitignore(root_path);

    // BinaryHeap with Reverse for "shortest path first" (most relevant)
    // Item: (Reverse(path_len), name, rel_path, is_dir)
    let mut heap: BinaryHeap<Reverse<(usize, String, String, bool)>> = BinaryHeap::new();
    let mut count = 0usize;
    // Cap total files visited to prevent runaway on huge repos
    let max_visit = 50_000usize;

    let mut stack: Vec<std::path::PathBuf> = vec![root_path.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if count >= max_visit {
            break;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            count += 1;
            if count >= max_visit {
                break;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden files/dirs
            if name.starts_with('.') {
                continue;
            }

            let full_path = entry.path();
            let rel_path = match full_path.strip_prefix(root_path) {
                Ok(r) => r.to_string_lossy().to_string(),
                Err(_) => continue,
            };

            let is_dir = full_path.is_dir();

            // Skip gitignored entries
            if is_gitignored(&rel_path, &name, is_dir, &gitignore_patterns) {
                continue;
            }

            // Recurse into directories
            if is_dir {
                stack.push(full_path.clone());
            }

            // Match: all query terms must appear in the lowercased path
            let lower = rel_path.to_lowercase();
            let matches = terms.iter().all(|t| lower.contains(t));
            if !matches {
                continue;
            }

            heap.push(Reverse((rel_path.len(), name.clone(), rel_path, is_dir)));

            // Keep heap bounded to avoid memory blow-up
            if heap.len() > limit * 4 {
                let mut temp: Vec<_> = heap.into_sorted_vec();
                temp.truncate(limit * 2);
                heap = temp.into_iter().collect();
            }
        }
    }

    // Extract top `limit` results (shortest paths first = most relevant)
    let mut results: Vec<_> = heap.into_sorted_vec();
    results.truncate(limit);

    results
        .into_iter()
        .map(|Reverse((_, name, path, is_dir))| FileSearchEntry { name, path, is_dir })
        .collect()
}

/// Load simple .gitignore patterns from the project root.
pub(super) fn load_gitignore(root: &std::path::Path) -> Vec<String> {
    let gitignore = root.join(".gitignore");
    match std::fs::read_to_string(gitignore) {
        Ok(content) => content
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check if a path matches any gitignore pattern (simplified matching).
pub(super) fn is_gitignored(rel_path: &str, name: &str, is_dir: bool, patterns: &[String]) -> bool {
    // Common dirs to always skip (performance)
    const ALWAYS_SKIP: &[&str] = &[
        "node_modules",
        "target",
        ".git",
        "__pycache__",
        ".next",
        "dist",
        "build",
        ".cache",
        ".turbo",
        "vendor",
    ];
    if is_dir && ALWAYS_SKIP.contains(&name) {
        return true;
    }

    for pattern in patterns {
        let pat = pattern.trim_end_matches('/');
        // Simple name match (e.g. "node_modules", "*.pyc")
        if pat.contains('*') {
            // Glob-style: *.ext
            if let Some(ext) = pat.strip_prefix("*.") {
                if name.ends_with(&format!(".{ext}")) {
                    return true;
                }
            }
        } else if pat.contains('/') {
            // Path-prefix match (e.g. "dist/", "build/output")
            if rel_path.starts_with(pat) || rel_path == pat {
                return true;
            }
        } else {
            // Plain name match
            if name == pat {
                return true;
            }
            // Also check if any path segment matches
            if rel_path.split('/').any(|seg| seg == pat) {
                return true;
            }
        }
    }
    false
}
