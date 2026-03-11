impl super::WebStateHandle {
    /// Record a file edit event for a session.
    /// Reads the file from disk, stores original snapshot on first edit,
    /// and records the edit with before/after content.
    pub async fn record_file_edit(
        &self,
        session_id: &str,
        file_path: &str,
        project_dir: Option<&std::path::Path>,
    ) {
        // Resolve absolute path
        let abs_path = if std::path::Path::new(file_path).is_absolute() {
            std::path::PathBuf::from(file_path)
        } else if let Some(dir) = project_dir {
            dir.join(file_path)
        } else {
            std::path::PathBuf::from(file_path)
        };

        // Read current file content
        let new_content = match tokio::fs::read_to_string(&abs_path).await {
            Ok(c) => c,
            Err(_) => return, // File doesn't exist or can't be read
        };

        // Check if we already have a snapshot (read lock first to avoid holding
        // a write lock across the git subprocess .await).
        let needs_original = {
            let inner = self.inner.read().await;
            inner
                .file_snapshots
                .get(session_id)
                .and_then(|snaps| snaps.get(file_path))
                .cloned()
        };

        let original_content = match needs_original {
            Some(existing) => existing,
            None => {
                // First edit: fetch git original content BEFORE acquiring write lock
                let original = Self::get_git_original(&abs_path, project_dir).await
                    .unwrap_or_else(|| new_content.clone());
                // Store snapshot under write lock (brief, no .await inside)
                let mut inner = self.inner.write().await;
                let snapshots = inner
                    .file_snapshots
                    .entry(session_id.to_string())
                    .or_default();
                // Double-check: another task may have inserted while we awaited
                if let Some(existing) = snapshots.get(file_path) {
                    existing.clone()
                } else {
                    snapshots.insert(file_path.to_string(), original.clone());
                    drop(inner); // release write lock early
                    original
                }
            }
        };

        // Record the edit (brief write lock, no .await inside)
        {
            let mut inner = self.inner.write().await;
            let edits = inner
                .file_edits
                .entry(session_id.to_string())
                .or_default();
            let index = edits.len();
            let timestamp = chrono::Utc::now().to_rfc3339();

            edits.push(super::FileEditRecord {
                path: file_path.to_string(),
                original_content,
                new_content,
                timestamp,
                index,
            });
        }
    }

    /// Try to get the original file content from git (HEAD version).
    async fn get_git_original(
        abs_path: &std::path::Path,
        project_dir: Option<&std::path::Path>,
    ) -> Option<String> {
        let dir = project_dir?;
        // Make path relative to project dir
        let rel_path = abs_path.strip_prefix(dir).ok()?;
        let output = tokio::process::Command::new("git")
            .arg("show")
            .arg(format!("HEAD:{}", rel_path.display()))
            .current_dir(dir)
            .output()
            .await
            .ok()?;
        if output.status.success() {
            String::from_utf8(output.stdout).ok()
        } else {
            None
        }
    }
}
