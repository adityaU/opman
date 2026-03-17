//! GitPanel — changes, log, diff, commit, branch management, AI actions, PR flow.
//! Matches React `GitPanel.tsx` with view stack navigation, AI review/commit/PR parity.

use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use web_sys::wasm_bindgen::JsCast;

use crate::components::icons::*;
use crate::hooks::use_panel_state::PanelState;
use crate::types::api::*;

// ── Utility: status color / label (matching React utils.ts) ────────

fn status_color(status: &str) -> &'static str {
    match status {
        "A" | "added" => "var(--color-success)",
        "D" | "deleted" => "var(--color-error)",
        "M" | "modified" => "var(--color-warning)",
        "R" | "renamed" => "var(--color-text)",
        "C" | "copied" => "var(--color-info)",
        "U" | "unmerged" => "var(--color-error)",
        "?" | "untracked" => "var(--color-text-muted)",
        _ => "var(--color-text-muted)",
    }
}

fn status_label(status: &str) -> &'static str {
    match status {
        "A" | "added" => "Added",
        "D" | "deleted" => "Deleted",
        "M" | "modified" => "Modified",
        "R" | "renamed" => "Renamed",
        "C" | "copied" => "Copied",
        "U" | "unmerged" => "Unmerged",
        "?" | "untracked" => "Untracked",
        _ => "Unknown",
    }
}

// ── View stack model ───────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum GitView {
    List,
    FileDiff { file: String, staged: bool },
    Commit { hash: String, short_hash: String },
}

/// Snapshot of git panel state for per-project caching.
#[derive(Clone, Debug)]
struct GitSnapshot {
    selected_repo: Option<String>,
    active_tab: String,
    view_stack: Vec<GitView>,
}

// ── Utility: relative time formatting ──────────────────────────────

fn format_relative_time(iso: &str) -> String {
    let now_ms = js_sys::Date::now(); // millis since epoch
    // Try to parse the ISO date
    let parsed = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
    let ts = parsed.get_time();
    if ts.is_nan() {
        return iso.to_string();
    }
    let diff_ms = now_ms - ts;
    let diff_mins = (diff_ms / 60_000.0).floor() as i64;
    if diff_mins < 1 {
        return "just now".to_string();
    }
    if diff_mins < 60 {
        return format!("{}m ago", diff_mins);
    }
    let diff_hours = diff_mins / 60;
    if diff_hours < 24 {
        return format!("{}h ago", diff_hours);
    }
    let diff_days = diff_hours / 24;
    if diff_days < 30 {
        return format!("{}d ago", diff_days);
    }
    let diff_months = diff_days / 30;
    if diff_months < 12 {
        return format!("{}mo ago", diff_months);
    }
    format!("{}y ago", diff_months / 12)
}

// ── Utility: split diff by file ────────────────────────────────────

fn split_diff_by_file(full_diff: &str) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();
    if full_diff.trim().is_empty() {
        return result;
    }
    // Split on "diff --git" boundaries
    let parts: Vec<&str> = full_diff.split("diff --git ").collect();
    for part in parts.iter().skip(1) {
        let chunk = format!("diff --git {}", part);
        // Extract file path from "diff --git a/foo b/bar"
        if let Some(first_line) = chunk.lines().next() {
            // parse "diff --git a/X b/Y" -> take Y
            let tokens: Vec<&str> = first_line.splitn(4, ' ').collect();
            if tokens.len() >= 4 {
                let b_path = tokens[3].strip_prefix("b/").unwrap_or(tokens[3]);
                result.push((b_path.to_string(), chunk));
            }
        }
    }
    result
}

// ── Utility: parse unified diff into old/new text (matching React parseUnifiedDiff) ──

fn parse_unified_diff(diff: &str) -> (String, String) {
    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();
    let mut in_diff = false;

    for line in diff.lines() {
        if line.starts_with("@@") {
            in_diff = true;
            continue;
        }
        if !in_diff {
            continue;
        }
        // Stop at next diff header
        if line.starts_with("diff --git") {
            break;
        }
        if let Some(rest) = line.strip_prefix('+') {
            new_lines.push(rest);
        } else if let Some(rest) = line.strip_prefix('-') {
            old_lines.push(rest);
        } else {
            // Context line (may start with ' ' or just be the line)
            let content = line.strip_prefix(' ').unwrap_or(line);
            old_lines.push(content);
            new_lines.push(content);
        }
    }

    (old_lines.join("\n"), new_lines.join("\n"))
}

// ── Utility: breadcrumb label for a git view ──────────────────────

fn breadcrumb_label(view: &GitView, tab: &str) -> String {
    match view {
        GitView::List => {
            if tab == "log" {
                "Log".to_string()
            } else {
                "Changes".to_string()
            }
        }
        GitView::FileDiff { file, staged: _ } => {
            file.split('/').last().unwrap_or(file).to_string()
        }
        GitView::Commit { hash: _, short_hash } => {
            short_hash.clone()
        }
    }
}

// ── GitPanel component ─────────────────────────────────────────────

#[component]
pub fn GitPanel(
    panels: PanelState,
    #[prop(optional)] on_send_to_ai: Option<Callback<String>>,
) -> impl IntoView {
    // Tab state
    let (active_tab, set_active_tab) = signal(String::from("changes"));

    // Repo state
    let (repos, set_repos) = signal(Vec::<GitRepoEntry>::new());
    let (selected_repo, set_selected_repo) = signal(Option::<String>::None);
    let (repo_dropdown_open, set_repo_dropdown_open) = signal(false);

    // Branch state
    let (branches, set_branches) = signal(Option::<GitBranchesResponse>::None);
    let (branch_dropdown_open, set_branch_dropdown_open) = signal(false);
    let (branch_filter, set_branch_filter) = signal(String::new());
    let (branches_loading, set_branches_loading) = signal(false);
    let (checking_out, set_checking_out) = signal(false);

    // NodeRefs for outside-click
    let repo_ref = NodeRef::<leptos::html::Div>::new();
    let branch_ref = NodeRef::<leptos::html::Div>::new();
    let breadcrumb_back_ref = NodeRef::<leptos::html::Div>::new();

    // Changes state
    let (status, set_status) = signal(Option::<GitStatusResponse>::None);
    let (status_loading, set_status_loading) = signal(false);

    // Log state
    let (log_entries, set_log_entries) = signal(Vec::<GitLogEntry>::new());
    let (log_loading, set_log_loading) = signal(false);

    // View stack
    let (view_stack, set_view_stack) = signal(vec![GitView::List]);
    let (diff_content, set_diff_content) = signal(String::new());
    let (diff_loading, set_diff_loading) = signal(false);
    let (commit_detail, set_commit_detail) = signal(Option::<GitShowResponse>::None);
    let (commit_loading, set_commit_loading) = signal(false);
    let (breadcrumb_dropdown, set_breadcrumb_dropdown) = signal(false);

    // Commit form
    let (commit_message, set_commit_message) = signal(String::new());
    let (committing, set_committing) = signal(false);

    // Section collapse signals
    let (staged_open, set_staged_open) = signal(true);
    let (unstaged_open, set_unstaged_open) = signal(true);
    let (untracked_open, set_untracked_open) = signal(true);

    // Commit detail: per-file expand/collapse
    let (expanded_files, set_expanded_files) = signal(HashSet::<String>::new());

    // Error message signal
    let (error_msg, set_error_msg) = signal(Option::<String>::None);

    // ── AI state (matching React useAIActions) ─────────────────────
    let (ai_review_loading, set_ai_review_loading) = signal(false);
    let (ai_commit_msg_loading, set_ai_commit_msg_loading) = signal(false);
    let (ai_pr_loading, set_ai_pr_loading) = signal(false);
    let (pr_branch_picker_open, set_pr_branch_picker_open) = signal(false);
    let (pr_modal_open, set_pr_modal_open) = signal(false);
    let (pr_modal_data, set_pr_modal_data) = signal(Option::<GitRangeDiffResponse>::None);

    // ── Pull / Stash / Gitignore state ─────────────────────────────
    let (pulling, set_pulling) = signal(false);
    let (stashing, set_stashing) = signal(false);
    let (stash_list_open, set_stash_list_open) = signal(false);
    let (stash_entries, set_stash_entries) = signal(Vec::<GitStashEntry>::new());
    let (gitignore_open, set_gitignore_open) = signal(false);
    let (gitignore_content, set_gitignore_content) = signal(String::new());
    let (gitignore_loading, set_gitignore_loading) = signal(false);
    let (gitignore_add_input, set_gitignore_add_input) = signal(String::new());

    // Current view
    let current_view = Memo::new(move |_| {
        view_stack.get().last().cloned().unwrap_or(GitView::List)
    });

    // ── Per-project snapshot cache ─────────────────────────────────
    let project_snapshots: StoredValue<HashMap<usize, GitSnapshot>> =
        StoredValue::new(HashMap::new());

    {
        let project_ctx = leptos::prelude::use_context::<crate::hooks::use_project_context::ProjectContext>();
        if let Some(ctx) = project_ctx {
            Effect::new(move |prev_idx: Option<usize>| {
                let new_idx = ctx.index.get();
                if let Some(old) = prev_idx {
                    if old != new_idx {
                        // Save current project's git state
                        let snap = GitSnapshot {
                            selected_repo: selected_repo.get_untracked(),
                            active_tab: active_tab.get_untracked(),
                            view_stack: view_stack.get_untracked(),
                        };
                        project_snapshots.update_value(|map| {
                            map.insert(old, snap);
                        });

                        // Restore new project's state or reset
                        let restored = project_snapshots
                            .with_value(|map| map.get(&new_idx).cloned());
                        if let Some(s) = restored {
                            set_selected_repo.set(s.selected_repo);
                            set_active_tab.set(s.active_tab);
                            set_view_stack.set(s.view_stack);
                        } else {
                            // Reset to defaults — repos will re-fetch via effect
                            set_selected_repo.set(None);
                            set_active_tab.set("changes".to_string());
                            set_view_stack.set(vec![GitView::List]);
                        }
                        // Clear transient state
                        set_breadcrumb_dropdown.set(false);
                        set_repo_dropdown_open.set(false);
                        set_branch_dropdown_open.set(false);
                        set_branch_filter.set(String::new());
                        set_diff_content.set(String::new());
                        set_commit_detail.set(None);
                        set_commit_message.set(String::new());
                        set_error_msg.set(None);
                        set_expanded_files.set(HashSet::new());
                        set_pr_branch_picker_open.set(false);
                        set_pr_modal_open.set(false);
                        set_pr_modal_data.set(None);
                    }
                }
                new_idx
            });
        }
    }

    // ── Error helpers ──────────────────────────────────────────────
    let show_error = move |msg: String| {
        set_error_msg.set(Some(msg));
        // Auto-clear after 5 seconds
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(5_000).await;
            set_error_msg.set(None);
        });
    };

    // ── Outside-click handlers (matching React useOutsideClick) ──────
    // Repo switcher outside-click
    Effect::new(move |_| {
        if !repo_dropdown_open.get() { return; }
        use wasm_bindgen::closure::Closure;
        let cb = Closure::<dyn Fn(web_sys::Event)>::new(move |e: web_sys::Event| {
            if let Some(el) = repo_ref.get() {
                if let Some(target) = e.target() {
                    let node: &web_sys::Node = el.unchecked_ref();
                    if !node.contains(Some(target.unchecked_ref())) {
                        set_repo_dropdown_open.set(false);
                    }
                }
            }
        });
        let doc = web_sys::window().unwrap().document().unwrap();
        let _ = doc.add_event_listener_with_callback("mousedown", cb.as_ref().unchecked_ref());
        let cb = send_wrapper::SendWrapper::new(cb);
        on_cleanup(move || {
            let doc = web_sys::window().unwrap().document().unwrap();
            let _ = doc.remove_event_listener_with_callback("mousedown", cb.as_ref().as_ref().unchecked_ref());
        });
    });

    // Branch switcher outside-click
    Effect::new(move |_| {
        if !branch_dropdown_open.get() { return; }
        use wasm_bindgen::closure::Closure;
        let cb = Closure::<dyn Fn(web_sys::Event)>::new(move |e: web_sys::Event| {
            if let Some(el) = branch_ref.get() {
                if let Some(target) = e.target() {
                    let node: &web_sys::Node = el.unchecked_ref();
                    if !node.contains(Some(target.unchecked_ref())) {
                        set_branch_dropdown_open.set(false);
                        set_branch_filter.set(String::new());
                    }
                }
            }
        });
        let doc = web_sys::window().unwrap().document().unwrap();
        let _ = doc.add_event_listener_with_callback("mousedown", cb.as_ref().unchecked_ref());
        let cb = send_wrapper::SendWrapper::new(cb);
        on_cleanup(move || {
            let doc = web_sys::window().unwrap().document().unwrap();
            let _ = doc.remove_event_listener_with_callback("mousedown", cb.as_ref().as_ref().unchecked_ref());
        });
    });

    // Breadcrumb dropdown outside-click
    Effect::new(move |_| {
        if !breadcrumb_dropdown.get() { return; }
        use wasm_bindgen::closure::Closure;
        let cb = Closure::<dyn Fn(web_sys::Event)>::new(move |e: web_sys::Event| {
            if let Some(el) = breadcrumb_back_ref.get() {
                if let Some(target) = e.target() {
                    let node: &web_sys::Node = el.unchecked_ref();
                    if !node.contains(Some(target.unchecked_ref())) {
                        set_breadcrumb_dropdown.set(false);
                    }
                }
            }
        });
        let doc = web_sys::window().unwrap().document().unwrap();
        let _ = doc.add_event_listener_with_callback("mousedown", cb.as_ref().unchecked_ref());
        let cb = send_wrapper::SendWrapper::new(cb);
        on_cleanup(move || {
            let doc = web_sys::window().unwrap().document().unwrap();
            let _ = doc.remove_event_listener_with_callback("mousedown", cb.as_ref().as_ref().unchecked_ref());
        });
    });

    // ── Fetch repos ────────────────────────────────────────────────
    Effect::new(move |prev: Option<()>| {
        if prev.is_none() {
            leptos::task::spawn_local(async move {
                match crate::api::git::git_repos().await {
                    Ok(resp) => {
                        if let Some(first) = resp.repos.first() {
                            set_selected_repo.set(Some(first.path.clone()));
                        }
                        set_repos.set(resp.repos);
                    }
                    Err(e) => {
                        log::error!("Failed to fetch repos: {}", e);
                        show_error(format!("Failed to fetch repos: {}", e));
                    }
                }
            });
        }
    });

    // ── Fetch status when repo selected ────────────────────────────
    Effect::new(move |_| {
        let repo = selected_repo.get();
        if let Some(repo) = repo {
            set_status_loading.set(true);
            let repo2 = repo.clone();
            leptos::task::spawn_local(async move {
                match crate::api::git::git_status(&repo).await {
                    Ok(resp) => set_status.set(Some(resp)),
                    Err(e) => log::error!("Failed to fetch status: {}", e),
                }
                set_status_loading.set(false);
            });

            // Also fetch branches
            leptos::task::spawn_local(async move {
                match crate::api::git::git_branches(&repo2).await {
                    Ok(resp) => set_branches.set(Some(resp)),
                    Err(e) => log::error!("Failed to fetch branches: {}", e),
                }
            });
        }
    });

    // ── Fetch log when log tab activated ───────────────────────────
    Effect::new(move |_| {
        let tab = active_tab.get();
        let repo = selected_repo.get();
        if tab == "log" {
            if let Some(repo) = repo {
                set_log_loading.set(true);
                leptos::task::spawn_local(async move {
                    match crate::api::git::git_log(&repo, Some(100)).await {
                        Ok(resp) => set_log_entries.set(resp.commits),
                        Err(e) => log::error!("Failed to fetch log: {}", e),
                    }
                    set_log_loading.set(false);
                });
            }
        }
    });

    // ── Navigation helpers ─────────────────────────────────────────
    let push_view = move |view: GitView| {
        set_breadcrumb_dropdown.set(false);
        set_view_stack.update(|stack| stack.push(view));
    };

    let pop_view = move |_| {
        set_breadcrumb_dropdown.set(false);
        set_view_stack.update(|stack| {
            if stack.len() > 1 {
                stack.pop();
            }
        });
    };

    let jump_to_view = move |index: usize| {
        set_breadcrumb_dropdown.set(false);
        set_view_stack.update(|stack| {
            if index < stack.len() {
                stack.truncate(index + 1);
            }
        });
    };

    // Open file diff
    let open_diff = move |file: String, staged: bool| {
        push_view(GitView::FileDiff { file: file.clone(), staged });
        set_diff_loading.set(true);
        let repo = selected_repo.get_untracked().unwrap_or_default();
        leptos::task::spawn_local(async move {
            match crate::api::git::git_diff(&repo, Some(&file), staged).await {
                Ok(resp) => set_diff_content.set(resp.diff),
                Err(e) => {
                    log::error!("Failed to fetch diff: {}", e);
                    set_diff_content.set(format!("Error: {}", e));
                }
            }
            set_diff_loading.set(false);
        });
    };

    // Open commit detail
    let open_commit = move |hash: String, short_hash: String| {
        push_view(GitView::Commit { hash: hash.clone(), short_hash });
        set_commit_loading.set(true);
        set_expanded_files.set(HashSet::new());
        let repo = selected_repo.get_untracked().unwrap_or_default();
        leptos::task::spawn_local(async move {
            match crate::api::git::git_show(&repo, &hash).await {
                Ok(resp) => set_commit_detail.set(Some(resp)),
                Err(e) => {
                    log::error!("Failed to fetch commit: {}", e);
                    set_commit_detail.set(None);
                }
            }
            set_commit_loading.set(false);
        });
    };

    // ── Git actions ────────────────────────────────────────────────
    let stage_file = move |path: String| {
        let repo = selected_repo.get_untracked().unwrap_or_default();
        leptos::task::spawn_local(async move {
            if let Err(e) = crate::api::git::git_stage(&repo, &path).await {
                log::error!("Stage failed: {}", e);
                show_error(format!("Stage failed: {}", e));
                return;
            }
            // Refresh status
            match crate::api::git::git_status(&repo).await {
                Ok(resp) => set_status.set(Some(resp)),
                Err(e) => log::error!("Status refresh failed: {}", e),
            }
        });
    };

    let unstage_file = move |path: String| {
        let repo = selected_repo.get_untracked().unwrap_or_default();
        leptos::task::spawn_local(async move {
            if let Err(e) = crate::api::git::git_unstage(&repo, &path).await {
                log::error!("Unstage failed: {}", e);
                show_error(format!("Unstage failed: {}", e));
                return;
            }
            match crate::api::git::git_status(&repo).await {
                Ok(resp) => set_status.set(Some(resp)),
                Err(e) => log::error!("Status refresh failed: {}", e),
            }
        });
    };

    let discard_file = move |path: String| {
        // Confirmation dialog
        let window = web_sys::window().unwrap();
        let confirmed = window
            .confirm_with_message(&format!("Discard changes to {}?", path))
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        let repo = selected_repo.get_untracked().unwrap_or_default();
        leptos::task::spawn_local(async move {
            if let Err(e) = crate::api::git::git_discard(&repo, &path).await {
                log::error!("Discard failed: {}", e);
                show_error(format!("Discard failed: {}", e));
                return;
            }
            match crate::api::git::git_status(&repo).await {
                Ok(resp) => set_status.set(Some(resp)),
                Err(e) => log::error!("Status refresh failed: {}", e),
            }
        });
    };

    // Stage all (loop over files one by one)
    let stage_all = move |_| {
        let repo = selected_repo.get_untracked().unwrap_or_default();
        let s = status.get_untracked();
        if let Some(s) = s {
            let paths: Vec<String> = s.unstaged.iter().chain(s.untracked.iter()).map(|f| f.path.clone()).collect();
            leptos::task::spawn_local(async move {
                for p in &paths {
                    if let Err(e) = crate::api::git::git_stage(&repo, p).await {
                        log::error!("Stage all failed on {}: {}", p, e);
                        show_error(format!("Stage failed: {}", e));
                        break;
                    }
                }
                match crate::api::git::git_status(&repo).await {
                    Ok(resp) => set_status.set(Some(resp)),
                    Err(e) => log::error!("Status refresh failed: {}", e),
                }
            });
        }
    };

    // Unstage all (loop over staged files)
    let unstage_all = move |_| {
        let repo = selected_repo.get_untracked().unwrap_or_default();
        let s = status.get_untracked();
        if let Some(s) = s {
            let paths: Vec<String> = s.staged.iter().map(|f| f.path.clone()).collect();
            leptos::task::spawn_local(async move {
                for p in &paths {
                    if let Err(e) = crate::api::git::git_unstage(&repo, p).await {
                        log::error!("Unstage all failed on {}: {}", p, e);
                        show_error(format!("Unstage failed: {}", e));
                        break;
                    }
                }
                match crate::api::git::git_status(&repo).await {
                    Ok(resp) => set_status.set(Some(resp)),
                    Err(e) => log::error!("Status refresh failed: {}", e),
                }
            });
        }
    };

    let do_commit = move |_| {
        let msg = commit_message.get_untracked();
        if msg.trim().is_empty() {
            return;
        }
        let repo = selected_repo.get_untracked().unwrap_or_default();
        set_committing.set(true);
        leptos::task::spawn_local(async move {
            match crate::api::git::git_commit(&repo, &msg).await {
                Ok(_resp) => {
                    set_commit_message.set(String::new());
                    // Refresh status
                    if let Ok(s) = crate::api::git::git_status(&repo).await {
                        set_status.set(Some(s));
                    }
                }
                Err(e) => {
                    log::error!("Commit failed: {}", e);
                    show_error(format!("Commit failed: {}", e));
                }
            }
            set_committing.set(false);
        });
    };

    let checkout_branch = move |branch: String| {
        let repo = selected_repo.get_untracked().unwrap_or_default();
        set_branch_dropdown_open.set(false);
        set_branch_filter.set(String::new());
        set_checking_out.set(true);
        leptos::task::spawn_local(async move {
            match crate::api::git::git_checkout(&repo, &branch).await {
                Ok(_) => {
                    // Refresh
                    if let Ok(s) = crate::api::git::git_status(&repo).await {
                        set_status.set(Some(s));
                    }
                    if let Ok(b) = crate::api::git::git_branches(&repo).await {
                        set_branches.set(Some(b));
                    }
                }
                Err(e) => {
                    log::error!("Checkout failed: {}", e);
                    show_error(format!("Checkout failed: {}", e));
                }
            }
            set_checking_out.set(false);
        });
    };

    // Refresh callback — refresh log when on log tab, status otherwise
    let refresh = move |_| {
        let repo = selected_repo.get_untracked();
        let tab = active_tab.get_untracked();
        if let Some(repo) = repo {
            if tab == "log" {
                set_log_loading.set(true);
                leptos::task::spawn_local(async move {
                    if let Ok(resp) = crate::api::git::git_log(&repo, Some(100)).await {
                        set_log_entries.set(resp.commits);
                    }
                    set_log_loading.set(false);
                });
            } else {
                set_status_loading.set(true);
                leptos::task::spawn_local(async move {
                    if let Ok(s) = crate::api::git::git_status(&repo).await {
                        set_status.set(Some(s));
                    }
                    set_status_loading.set(false);
                });
            }
        }
    };

    // Helper: refresh just status (used after pull/stash/etc.)
    let refresh_status = move |repo: String| {
        leptos::task::spawn_local(async move {
            if let Ok(s) = crate::api::git::git_status(&repo).await {
                set_status.set(Some(s));
            }
        });
    };

    // ── Fetch branch list when PR branch picker opens (match React useEffect) ──
    Effect::new(move |_| {
        if pr_branch_picker_open.get() {
            let repo = selected_repo.get_untracked();
            if let Some(repo) = repo {
                set_branches_loading.set(true);
                leptos::task::spawn_local(async move {
                    match crate::api::git::git_branches(&repo).await {
                        Ok(resp) => set_branches.set(Some(resp)),
                        Err(e) => log::error!("Failed to fetch branches for PR picker: {}", e),
                    }
                    set_branches_loading.set(false);
                });
            }
        }
    });

    // ── AI action handlers (matching React useAIActions) ───────────
    let handle_ai_review = move |_| {
        let send = on_send_to_ai;
        let send = match send {
            Some(cb) => cb,
            None => return,
        };
        let repo = selected_repo.get_untracked().unwrap_or_default();
        let s = status.get_untracked();
        let staged_count = s.as_ref().map(|s| s.staged.len()).unwrap_or(0);
        let unstaged_count = s.as_ref().map(|s| s.unstaged.len()).unwrap_or(0);
        if staged_count == 0 && unstaged_count == 0 {
            show_error("No changes to review".to_string());
            return;
        }
        set_ai_review_loading.set(true);
        leptos::task::spawn_local(async move {
            // Fetch staged + unstaged diffs in parallel
            let (staged_res, unstaged_res) = futures::join!(
                crate::api::git::git_diff(&repo, None, true),
                crate::api::git::git_diff(&repo, None, false),
            );
            let staged_diff = staged_res.map(|r| r.diff).unwrap_or_default();
            let unstaged_diff = unstaged_res.map(|r| r.diff).unwrap_or_default();
            let combined = format!("{}{}", staged_diff, unstaged_diff);
            if combined.trim().is_empty() {
                show_error("No changes to review".to_string());
            } else {
                let file_summary = format!("{} staged, {} unstaged", staged_count, unstaged_count);
                let prompt = format!(
                    "Review my current git changes ({} files). Use the available tools to read the git diff, then provide a thorough code review covering potential bugs, code quality, and suggestions for improvement.",
                    file_summary
                );
                send.run(prompt);
            }
            set_ai_review_loading.set(false);
        });
    };

    let handle_ai_commit_msg = move |_| {
        let send = on_send_to_ai;
        let send = match send {
            Some(cb) => cb,
            None => return,
        };
        let s = status.get_untracked();
        let staged = s.as_ref().map(|s| s.staged.clone()).unwrap_or_default();
        if staged.is_empty() {
            show_error("Stage some files first".to_string());
            return;
        }
        let repo = selected_repo.get_untracked().unwrap_or_default();
        set_ai_commit_msg_loading.set(true);
        leptos::task::spawn_local(async move {
            match crate::api::git::git_diff(&repo, None, true).await {
                Ok(resp) => {
                    if resp.diff.trim().is_empty() {
                        show_error("No staged changes found".to_string());
                    } else {
                        let file_list: String = staged.iter()
                            .map(|f| format!("  {} {}", f.status, f.path))
                            .collect::<Vec<_>>()
                            .join("\n");
                        let prompt = format!(
                            "Generate a concise, well-structured git commit message for the following staged changes.\n\nStaged files:\n{}\n\nDiff:\n```diff\n{}\n```\n\nWrite a commit message following conventional commit format (e.g. feat:, fix:, refactor:). Include a brief subject line (max 72 chars) and an optional body with bullet points if needed. Return ONLY the commit message, nothing else.",
                            file_list, resp.diff
                        );
                        send.run(prompt);
                    }
                }
                Err(e) => {
                    show_error(format!("Failed to fetch staged diff: {}", e));
                }
            }
            set_ai_commit_msg_loading.set(false);
        });
    };

    let open_pr_branch_picker = move |_| {
        set_pr_branch_picker_open.set(true);
    };

    let handle_ai_pr_description = move |base_branch: String| {
        let send = on_send_to_ai;
        let send = match send {
            Some(cb) => cb,
            None => return,
        };
        set_pr_branch_picker_open.set(false);
        let repo = selected_repo.get_untracked().unwrap_or_default();
        set_ai_pr_loading.set(true);
        let base = if base_branch.is_empty() { None } else { Some(base_branch) };
        leptos::task::spawn_local(async move {
            match crate::api::git::git_range_diff(&repo, base.as_deref()).await {
                Ok(resp) => {
                    if resp.diff.trim().is_empty() && resp.commits.is_empty() {
                        show_error("No commits found relative to base branch".to_string());
                    } else {
                        // Build commit list
                        let commits_str: String = resp.commits.iter()
                            .map(|c| format!("  - {} {}", &c.hash[..7.min(c.hash.len())], c.message))
                            .collect::<Vec<_>>()
                            .join("\n");
                        // Truncate diff to 8000 chars (matching React)
                        let diff_truncated = if resp.diff.len() > 8000 {
                            format!("{}... (diff truncated)", resp.diff.chars().take(8000).collect::<String>())
                        } else {
                            resp.diff.clone()
                        };
                        let prompt = format!(
                            "Draft a pull request description for merging `{}` into `{}`.\n\nCommits ({}):\n{}\n\nFiles changed: {}\n\nDiff:\n```diff\n{}\n```\n\nWrite a clear PR description with:\n- A concise title\n- A summary section with bullet points\n- Any notable changes or breaking changes\n- Testing notes if applicable",
                            resp.branch, resp.base, resp.commits.len(), commits_str, resp.files_changed, diff_truncated
                        );
                        send.run(prompt);
                        // Show PR modal with context
                        set_pr_modal_data.set(Some(resp));
                        set_pr_modal_open.set(true);
                    }
                }
                Err(e) => {
                    show_error(format!("Failed to fetch range diff: {}", e));
                }
            }
            set_ai_pr_loading.set(false);
        });
    };

    // Commit detail: toggle file accordion
    let toggle_file_accordion = move |path: String| {
        set_expanded_files.update(|set| {
            if set.contains(&path) {
                set.remove(&path);
            } else {
                set.insert(path);
            }
        });
    };

    let expand_all_files = move |_| {
        if let Some(cd) = commit_detail.get_untracked() {
            set_expanded_files.set(cd.files.iter().map(|f| f.path.clone()).collect());
        }
    };

    let collapse_all_files = move |_| {
        set_expanded_files.set(HashSet::new());
    };

    // ── Pull handler ───────────────────────────────────────────────
    let handle_pull = move |_| {
        let repo = selected_repo.get_untracked().unwrap_or_default();
        if repo.is_empty() {
            return;
        }
        set_pulling.set(true);
        leptos::task::spawn_local(async move {
            match crate::api::git::git_pull(&repo, None, None).await {
                Ok(resp) => {
                    if !resp.success {
                        show_error(format!("Pull failed: {}", resp.output));
                    }
                    // Refresh status after pull
                    refresh_status(repo);
                }
                Err(e) => show_error(format!("Pull failed: {}", e)),
            }
            set_pulling.set(false);
        });
    };

    // ── Stash push handler ─────────────────────────────────────────
    let handle_stash_push = move |_| {
        let repo = selected_repo.get_untracked().unwrap_or_default();
        if repo.is_empty() {
            return;
        }
        set_stashing.set(true);
        leptos::task::spawn_local(async move {
            match crate::api::git::git_stash_push(&repo, None).await {
                Ok(resp) => {
                    if !resp.success {
                        show_error(format!("Stash failed: {}", resp.output));
                    }
                    refresh_status(repo);
                }
                Err(e) => show_error(format!("Stash failed: {}", e)),
            }
            set_stashing.set(false);
        });
    };

    // ── Stash pop handler ──────────────────────────────────────────
    let handle_stash_pop = move |_| {
        let repo = selected_repo.get_untracked().unwrap_or_default();
        if repo.is_empty() {
            return;
        }
        set_stashing.set(true);
        leptos::task::spawn_local(async move {
            match crate::api::git::git_stash_pop(&repo).await {
                Ok(resp) => {
                    if !resp.success {
                        show_error(format!("Stash pop failed: {}", resp.output));
                    }
                    refresh_status(repo);
                }
                Err(e) => show_error(format!("Stash pop failed: {}", e)),
            }
            set_stashing.set(false);
        });
    };

    // ── Stash list toggle ──────────────────────────────────────────
    let toggle_stash_list = move |_| {
        let opening = !stash_list_open.get_untracked();
        set_stash_list_open.set(opening);
        if opening {
            let repo = selected_repo.get_untracked().unwrap_or_default();
            if !repo.is_empty() {
                leptos::task::spawn_local(async move {
                    match crate::api::git::git_stash_list(&repo).await {
                        Ok(resp) => set_stash_entries.set(resp.entries),
                        Err(e) => {
                            log::error!("Failed to list stashes: {}", e);
                            set_stash_entries.set(vec![]);
                        }
                    }
                });
            }
        }
    };

    // ── Stash drop handler ─────────────────────────────────────────
    let handle_stash_drop = move |stash_ref: String| {
        let repo = selected_repo.get_untracked().unwrap_or_default();
        if repo.is_empty() {
            return;
        }
        leptos::task::spawn_local(async move {
            match crate::api::git::git_stash_drop(&repo, &stash_ref).await {
                Ok(resp) => {
                    if !resp.success {
                        show_error(format!("Stash drop failed: {}", resp.output));
                    }
                    // Refresh the stash list
                    match crate::api::git::git_stash_list(&repo).await {
                        Ok(r) => set_stash_entries.set(r.entries),
                        Err(_) => set_stash_entries.set(vec![]),
                    }
                }
                Err(e) => show_error(format!("Stash drop failed: {}", e)),
            }
        });
    };

    // ── Gitignore toggle ───────────────────────────────────────────
    let toggle_gitignore = move |_| {
        let opening = !gitignore_open.get_untracked();
        set_gitignore_open.set(opening);
        if opening {
            let repo = selected_repo.get_untracked().unwrap_or_default();
            if !repo.is_empty() {
                set_gitignore_loading.set(true);
                leptos::task::spawn_local(async move {
                    match crate::api::git::git_gitignore_list(&repo).await {
                        Ok(resp) => set_gitignore_content.set(resp.content),
                        Err(e) => {
                            log::error!("Failed to load .gitignore: {}", e);
                            set_gitignore_content.set(String::new());
                        }
                    }
                    set_gitignore_loading.set(false);
                });
            }
        }
    };

    // ── Gitignore add handler ──────────────────────────────────────
    let handle_gitignore_add = move |_| {
        let input = gitignore_add_input.get_untracked();
        let patterns: Vec<String> = input.lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        if patterns.is_empty() {
            return;
        }
        let repo = selected_repo.get_untracked().unwrap_or_default();
        if repo.is_empty() {
            return;
        }
        set_gitignore_loading.set(true);
        leptos::task::spawn_local(async move {
            let refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
            match crate::api::git::git_gitignore_add(&repo, &refs).await {
                Ok(resp) => {
                    set_gitignore_content.set(resp.content);
                    set_gitignore_add_input.set(String::new());
                }
                Err(e) => show_error(format!("Failed to update .gitignore: {}", e)),
            }
            set_gitignore_loading.set(false);
        });
    };

    view! {
        <div class="git-panel flex flex-col h-full bg-bg-panel">
            // Header
            <div class="git-panel-header">
                // RepoSwitcher — only when repos.length > 1
                {move || {
                    if repos.get().len() > 1 {
                        Some(view! {
                            <div class="git-repo-switcher" node_ref=repo_ref>
                                <button
                                    class="git-repo-toggle"
                                    on:click=move |_| set_repo_dropdown_open.update(|v| *v = !*v)
                                >
                                    <IconGitFork size=12 />
                                    <span class="git-repo-label">
                                        {move || {
                                            let sel = selected_repo.get();
                                            repos.get()
                                                .iter()
                                                .find(|r| Some(&r.path) == sel.as_ref())
                                                .map(|r| r.name.clone())
                                                .unwrap_or_else(|| "Select repo".to_string())
                                        }}
                                    </span>
                                    <span class=move || if repo_dropdown_open.get() { "rotated" } else { "" }>
                                        <IconChevronDown size=10 />
                                    </span>
                                </button>
                                {move || {
                                    if repo_dropdown_open.get() {
                                        let sel = selected_repo.get();
                                        Some(view! {
                                            <div class="git-repo-dropdown">
                                                {repos.get().iter().map(|r| {
                                                    let path = r.path.clone();
                                                    let name = r.name.clone();
                                                    let branch = r.branch.clone();
                                                    let total_changes = r.staged_count + r.unstaged_count + r.untracked_count;
                                                    let is_current = Some(&r.path) == sel.as_ref();
                                                    let item_class = if is_current { "git-repo-item current" } else { "git-repo-item" };
                                                    view! {
                                                        <button
                                                            class=item_class
                                                            on:click=move |_| {
                                                                set_selected_repo.set(Some(path.clone()));
                                                                set_repo_dropdown_open.set(false);
                                                                // Reset view stack on repo change (matching React)
                                                                set_view_stack.set(vec![GitView::List]);
                                                                set_breadcrumb_dropdown.set(false);
                                                            }
                                                        >
                                                            <div class="git-repo-item-main">
                                                                <span class="git-repo-item-name">{name.clone()}</span>
                                                                <span class="git-repo-item-branch">{branch.clone()}</span>
                                                            </div>
                                                            <div class="git-repo-item-meta">
                                                                {if total_changes > 0 {
                                                                    Some(view! { <span class="git-repo-item-changes">{total_changes}</span> })
                                                                } else {
                                                                    None
                                                                }}
                                                                {if is_current {
                                                                    Some(view! { <IconCheck size=12 /> })
                                                                } else {
                                                                    None
                                                                }}
                                                            </div>
                                                        </button>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        })
                                    } else {
                                        None
                                    }
                                }}
                            </div>
                        })
                    } else {
                        None
                    }
                }}

                // Toolbar row: breadcrumb + branch switcher
                <div class="git-panel-toolbar-row">
                    // Breadcrumb navigation (always render trail; back button only when stack > 1)
                    {move || {
                        let stack = view_stack.get();
                        let tab = active_tab.get();
                        let stack_len = stack.len();
                        view! {
                            <div class="git-breadcrumb">
                                // Back button group — only when stack > 1
                                {if stack_len > 1 {
                                    Some(view! {
                                        <div class="git-breadcrumb-back" node_ref=breadcrumb_back_ref>
                                            <button
                                                class="git-back-btn"
                                                on:click=pop_view
                                                title="Back"
                                            >
                                                <IconChevronLeft size=14 />
                                            </button>

                                            // Dropdown toggle (when stack depth > 2)
                                            {if stack_len > 2 {
                                                Some(view! {
                                                    <button
                                                        class="git-back-dropdown-btn"
                                                        on:click=move |_| set_breadcrumb_dropdown.update(|v| *v = !*v)
                                                        title="View history"
                                                    >
                                                        <IconChevronDown size=12 />
                                                    </button>
                                                    {move || {
                                                        if breadcrumb_dropdown.get() {
                                                            let stack_snap = view_stack.get();
                                                            let tab_snap = active_tab.get();
                                                            // Exclude current (last) item
                                                            let items: Vec<(usize, String)> = stack_snap.iter().enumerate()
                                                                .take(stack_snap.len().saturating_sub(1))
                                                                .map(|(i, v)| (i, breadcrumb_label(v, &tab_snap)))
                                                                .collect();
                                                            Some(view! {
                                                                <div class="git-breadcrumb-dropdown">
                                                                    {items.into_iter().map(|(i, label)| {
                                                                        view! {
                                                                            <button
                                                                                class="git-breadcrumb-dropdown-item"
                                                                                on:click=move |_| jump_to_view(i)
                                                                            >
                                                                                {label}
                                                                            </button>
                                                                        }
                                                                    }).collect::<Vec<_>>()}
                                                                </div>
                                                            })
                                                        } else {
                                                            None
                                                        }
                                                    }}
                                                })
                                            } else {
                                                None
                                            }}
                                        </div>
                                    })
                                } else {
                                    None
                                }}

                                // Breadcrumb trail (always rendered)
                                <div class="git-breadcrumb-trail">
                                    {stack.iter().enumerate().map(|(i, v)| {
                                        let label = breadcrumb_label(v, &tab);
                                        let is_last = i == stack_len - 1;
                                        if is_last {
                                            view! {
                                                <>
                                                    {if i > 0 {
                                                        Some(view! { <span class="git-breadcrumb-sep">"/"</span> })
                                                    } else {
                                                        None
                                                    }}
                                                    <span class="git-breadcrumb-current">{label}</span>
                                                </>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <>
                                                    {if i > 0 {
                                                        Some(view! { <span class="git-breadcrumb-sep">"/"</span> })
                                                    } else {
                                                        None
                                                    }}
                                                    <button
                                                        class="git-breadcrumb-link"
                                                        on:click=move |_| jump_to_view(i)
                                                    >
                                                        {label}
                                                    </button>
                                                </>
                                            }.into_any()
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            </div>
                        }
                    }}

                    // Branch switcher (includes refresh button)
                    <div class="git-panel-branch" node_ref=branch_ref>
                        <button
                            class="git-branch-toggle"
                            disabled=move || checking_out.get()
                            on:click=move |_| {
                                let was_open = branch_dropdown_open.get_untracked();
                                set_branch_dropdown_open.set(!was_open);
                                if was_open {
                                    // Closing — clear filter
                                    set_branch_filter.set(String::new());
                                } else {
                                    // Opening — clear filter + fetch branches
                                    set_branch_filter.set(String::new());
                                    if let Some(repo) = selected_repo.get_untracked() {
                                        set_branches_loading.set(true);
                                        leptos::task::spawn_local(async move {
                                            match crate::api::git::git_branches(&repo).await {
                                                Ok(resp) => set_branches.set(Some(resp)),
                                                Err(e) => log::error!("Failed to fetch branches: {}", e),
                                            }
                                            set_branches_loading.set(false);
                                        });
                                    }
                                }
                            }
                        >
                            <IconGitBranch size=12 />
                            <span>
                                {move || {
                                    if checking_out.get() {
                                        "Switching...".to_string()
                                    } else {
                                        branches.get()
                                            .map(|b| b.current.clone())
                                            .or_else(|| status.get().map(|s| s.branch.clone()))
                                            .unwrap_or_else(|| "...".to_string())
                                    }
                                }}
                            </span>
                            <IconChevronDown size=10 />
                        </button>
                        {move || {
                            if branch_dropdown_open.get() {
                                if branches_loading.get() {
                                    return Some(view! {
                                        <div class="git-branch-dropdown">
                                            <div class="git-branch-loading">
                                                <IconLoader2 size=14 />
                                                <span>"Loading..."</span>
                                            </div>
                                        </div>
                                    }.into_any());
                                }
                                let br = branches.get();
                                if let Some(br) = br {
                                    let filter_lower = branch_filter.get().to_lowercase();
                                    let filtered_local: Vec<String> = br.local.iter()
                                        .filter(|b| filter_lower.is_empty() || b.to_lowercase().contains(&filter_lower))
                                        .cloned()
                                        .collect();
                                    let filtered_remote: Vec<String> = br.remote.iter()
                                        .filter(|b| filter_lower.is_empty() || b.to_lowercase().contains(&filter_lower))
                                        .cloned()
                                        .collect();
                                    let no_matches = filtered_local.is_empty() && filtered_remote.is_empty();
                                    let current = br.current.clone();
                                    Some(view! {
                                        <div class="git-branch-dropdown">
                                            <div class="p-1 border-b border-border-subtle">
                                                <input
                                                    type="text"
                                                    class="git-branch-filter"
                                                    placeholder="Filter branches..."
                                                    prop:value=move || branch_filter.get()
                                                    on:input=move |e| set_branch_filter.set(event_target_value(&e))
                                                />
                                            </div>
                                            <div class="git-branch-list">
                                                // Local branches section — no section label
                                                {if !filtered_local.is_empty() {
                                                    let current2 = current.clone();
                                                    Some(view! {
                                                        <>
                                                            {filtered_local.iter().map(|b| {
                                                                let branch = b.clone();
                                                                let is_current = b == &current2;
                                                                let item_class = if is_current { "git-branch-item current" } else { "git-branch-item" };
                                                                    view! {
                                                                    <button
                                                                        class=item_class
                                                                        on:click=move |_| checkout_branch(branch.clone())
                                                                        disabled=move || is_current || checking_out.get()
                                                                    >
                                                                        <span class="git-branch-item-name">{b.clone()}</span>
                                                                        {if is_current {
                                                                            Some(view! { <IconCheck size=12 /> })
                                                                        } else {
                                                                            None
                                                                        }}
                                                                    </button>
                                                                }
                                                            }).collect::<Vec<_>>()}
                                                        </>
                                                    })
                                                } else {
                                                    None
                                                }}
                                                // Remote branches section — has label
                                                {if !filtered_remote.is_empty() {
                                                    Some(view! {
                                                        <div class="git-branch-section-label">"Remote"</div>
                                                        {filtered_remote.iter().map(|b| {
                                                            let branch = b.clone();
                                                            view! {
                                                                <button
                                                                    class="git-branch-item remote"
                                                                    on:click=move |_| checkout_branch(branch.clone())
                                                                >
                                                                    <span class="git-branch-item-name">{b.clone()}</span>
                                                                </button>
                                                            }
                                                        }).collect::<Vec<_>>()}
                                                    })
                                                } else {
                                                    None
                                                }}
                                                // No matching branches
                                                {if no_matches {
                                                    Some(view! {
                                                        <div class="git-branch-empty">"No matching branches"</div>
                                                    })
                                                } else {
                                                    None
                                                }}
                                            </div>
                                        </div>
                                    }.into_any())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }}
                        // Refresh button inside git-panel-branch
                        <button
                            class=move || {
                                if status_loading.get() || log_loading.get() {
                                    "git-panel-refresh spin"
                                } else {
                                    "git-panel-refresh"
                                }
                            }
                            title="Refresh"
                            on:click=refresh
                        >
                            <IconRefreshCw size=12 />
                        </button>
                    </div>
                </div>

                // Tab bar — only show when in list view (matching React GitTabBar)
                {move || {
                    if current_view.get() == GitView::List {
                        Some(view! {
                            <div class="git-panel-tabs">
                                <button
                                    class=move || {
                                        if active_tab.get() == "changes" {
                                            "git-tab active"
                                        } else {
                                            "git-tab"
                                        }
                                    }
                                    on:click=move |_| {
                                        set_active_tab.set("changes".to_string());
                                    }
                                >
                                    "Changes"
                                    {move || {
                                        status.get().map(|s| {
                                            let count = s.staged.len() + s.unstaged.len() + s.untracked.len();
                                            if count > 0 {
                                                Some(view! {
                                                    <span class="git-tab-badge">
                                                        {count}
                                                    </span>
                                                })
                                            } else {
                                                None
                                            }
                                        }).flatten()
                                    }}
                                </button>
                                <button
                                    class=move || {
                                        if active_tab.get() == "log" {
                                            "git-tab active"
                                        } else {
                                            "git-tab"
                                        }
                                    }
                                    on:click=move |_| {
                                        set_active_tab.set("log".to_string());
                                        set_view_stack.set(vec![GitView::List]);
                                        set_breadcrumb_dropdown.set(false);
                                    }
                                >
                                    <IconHistory size=12 />
                                    " Log"
                                </button>
                            </div>
                        })
                    } else {
                        None
                    }
                }}
            </div>

            // Git action bar (pull / stash / gitignore) — only in list view changes tab
            {move || {
                if current_view.get() != GitView::List || active_tab.get() != "changes" {
                    return None;
                }
                let has_repo = selected_repo.get().is_some();
                if !has_repo {
                    return None;
                }
                Some(view! {
                    <div class="git-actions-bar flex items-center gap-1 px-2 py-1 border-b border-border-subtle text-xs">
                        // Pull button
                        <button
                            class="git-action-btn flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-bg-element/50 text-text-muted hover:text-text transition-colors"
                            title="Pull from remote"
                            disabled=move || pulling.get()
                            on:click=handle_pull
                        >
                            {move || if pulling.get() {
                                view! { <IconLoader2 size=12 class="spin" /> }.into_any()
                            } else {
                                view! { <IconDownload size=12 /> }.into_any()
                            }}
                            <span>"Pull"</span>
                        </button>
                        // Stash push
                        <button
                            class="git-action-btn flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-bg-element/50 text-text-muted hover:text-text transition-colors"
                            title="Stash changes"
                            disabled=move || stashing.get()
                            on:click=handle_stash_push
                        >
                            {move || if stashing.get() {
                                view! { <IconLoader2 size=12 class="spin" /> }.into_any()
                            } else {
                                view! { <IconLayers size=12 /> }.into_any()
                            }}
                            <span>"Stash"</span>
                        </button>
                        // Stash pop
                        <button
                            class="git-action-btn flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-bg-element/50 text-text-muted hover:text-text transition-colors"
                            title="Pop stash"
                            disabled=move || stashing.get()
                            on:click=handle_stash_pop
                        >
                            <IconRotateCcw size=12 />
                            <span>"Pop"</span>
                        </button>
                        // Stash list toggle
                        <button
                            class=move || {
                                if stash_list_open.get() {
                                    "git-action-btn flex items-center gap-1 px-1.5 py-0.5 rounded bg-bg-element/50 text-text transition-colors"
                                } else {
                                    "git-action-btn flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-bg-element/50 text-text-muted hover:text-text transition-colors"
                                }
                            }
                            title="List stashes"
                            on:click=toggle_stash_list
                        >
                            <IconHistory size=12 />
                        </button>
                        // Spacer
                        <div class="flex-1"></div>
                        // Gitignore toggle
                        <button
                            class=move || {
                                if gitignore_open.get() {
                                    "git-action-btn flex items-center gap-1 px-1.5 py-0.5 rounded bg-bg-element/50 text-text transition-colors"
                                } else {
                                    "git-action-btn flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-bg-element/50 text-text-muted hover:text-text transition-colors"
                                }
                            }
                            title=".gitignore"
                            on:click=toggle_gitignore
                        >
                            <IconFileCode size=12 />
                            <span>".gitignore"</span>
                        </button>
                    </div>

                    // Stash list dropdown
                    {move || {
                        if !stash_list_open.get() {
                            return None;
                        }
                        let entries = stash_entries.get();
                        Some(view! {
                            <div class="git-stash-list border-b border-border-subtle bg-background/50 px-2 py-1">
                                {if entries.is_empty() {
                                    view! {
                                        <div class="text-text-muted text-xs py-2 text-center">"No stashes"</div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="space-y-0.5">
                                            {entries.iter().map(|e| {
                                                let ref_drop = e.reference.clone();
                                                let ref_display = e.reference.clone();
                                                let msg = e.message.clone();
                                                view! {
                                                    <div class="flex items-center gap-2 text-xs py-0.5 group">
                                                        <span class="text-primary font-mono text-[10px] flex-shrink-0">{ref_display}</span>
                                                        <span class="text-text truncate flex-1">{msg}</span>
                                                        <button
                                                            class="opacity-0 group-hover:opacity-100 text-text-muted hover:text-error text-[10px]"
                                                            title="Drop stash"
                                                            on:click=move |_| handle_stash_drop(ref_drop.clone())
                                                        >
                                                            <IconTrash2 size=11 />
                                                        </button>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    }.into_any()
                                }}
                            </div>
                        })
                    }}

                    // Gitignore panel
                    {move || {
                        if !gitignore_open.get() {
                            return None;
                        }
                        Some(view! {
                            <div class="git-gitignore-panel border-b border-border-subtle bg-background/50 px-2 py-1.5">
                                {move || {
                                    if gitignore_loading.get() {
                                        return view! {
                                            <div class="flex items-center gap-1 text-xs text-text-muted py-1">
                                                <IconLoader2 size=12 class="spin" />
                                                <span>"Loading..."</span>
                                            </div>
                                        }.into_any();
                                    }
                                    let content = gitignore_content.get();
                                    view! {
                                        <div>
                                            <pre class="text-[10px] font-mono text-text-muted max-h-32 overflow-y-auto mb-1 whitespace-pre-wrap">{content}</pre>
                                            <div class="flex items-center gap-1">
                                                <input
                                                    type="text"
                                                    class="flex-1 bg-background text-text text-xs px-1.5 py-0.5 rounded border border-border-subtle focus:border-primary outline-none"
                                                    placeholder="Add pattern (e.g. *.log)"
                                                    prop:value=move || gitignore_add_input.get()
                                                    on:input=move |e| set_gitignore_add_input.set(event_target_value(&e))
                                                    on:keydown=move |e: leptos::ev::KeyboardEvent| {
                                                        if e.key() == "Enter" {
                                                            e.prevent_default();
                                                            handle_gitignore_add(());
                                                        }
                                                    }
                                                />
                                                <button
                                                    class="text-xs px-1.5 py-0.5 rounded bg-primary text-white hover:bg-primary/80 transition-colors disabled:opacity-50"
                                                    disabled=move || gitignore_add_input.get().trim().is_empty() || gitignore_loading.get()
                                                    on:click=move |_| handle_gitignore_add(())
                                                >
                                                    "Add"
                                                </button>
                                            </div>
                                        </div>
                                    }.into_any()
                                }}
                            </div>
                        })
                    }}
                })
            }}

            // Content area
            <div class="git-content flex-1 overflow-y-auto">
                {move || {
                    match current_view.get() {
                        GitView::FileDiff { file, staged } => {
                            if diff_loading.get() {
                                view! {
                                    <div class="git-loading"><IconLoader2 size=18 /></div>
                                }.into_any()
                            } else {
                                let dc = diff_content.get();
                                let is_empty = dc.trim().is_empty();
                                view! {
                                    <div class="git-diff-fullview p-3">
                                        <div class="git-diff-header flex items-center gap-2 mb-2 text-xs">
                                            <IconFileText size=12 />
                                            <span class="text-text font-medium">{file.clone()}</span>
                                            <span class=move || {
                                                if staged {
                                                    "git-diff-type text-success text-[10px] px-1 rounded bg-success/10"
                                                } else {
                                                    "git-diff-type text-warning text-[10px] px-1 rounded bg-warning/10"
                                                }
                                            }>
                                                {if staged { "staged" } else { "unstaged" }}
                                            </span>
                                        </div>
                                        <div class="git-diff-body">
                                            {if is_empty {
                                                view! {
                                                    <div class="git-empty flex items-center justify-center py-8 text-text-muted text-sm">
                                                        "No diff available"
                                                    </div>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <pre class="text-xs font-mono leading-5 overflow-x-auto whitespace-pre">
                                                        {dc.lines().map(|line| {
                                                            let cls = if line.starts_with('+') && !line.starts_with("+++") {
                                                                "text-success bg-success/5"
                                                            } else if line.starts_with('-') && !line.starts_with("---") {
                                                                "text-error bg-error/5"
                                                            } else if line.starts_with("@@") {
                                                                "text-info"
                                                            } else {
                                                                "text-text"
                                                            };
                                                            let line_str = format!("{}\n", line);
                                                            view! { <span class=cls>{line_str}</span> }
                                                        }).collect::<Vec<_>>()}
                                                    </pre>
                                                }.into_any()
                                            }}
                                        </div>
                                    </div>
                                }.into_any()
                            }
                        }
                        GitView::Commit { hash: _hash, short_hash: _short_hash } => {
                            if commit_loading.get() {
                                view! {
                                    <div class="git-loading"><IconLoader2 size=18 /></div>
                                }.into_any()
                            } else {
                                match commit_detail.get() {
                                    None => view! {
                                        <div class="flex items-center justify-center h-full text-text-muted text-sm">
                                            "Failed to load commit"
                                        </div>
                                    }.into_any(),
                                    Some(cd) => {
                                        let per_file_diffs = split_diff_by_file(&cd.diff);
                                        let file_count = cd.files.len();
                                        view! {
                                            <div class="git-commit-detail p-3">
                                                // Commit metadata
                                                <div class="git-commit-meta mb-3">
                                                    <div class="git-commit-meta-hash text-primary font-mono text-[10px] mb-0.5">
                                                        {cd.hash[..10.min(cd.hash.len())].to_string()}
                                                    </div>
                                                    <div class="git-commit-meta-message text-text font-medium text-sm mb-1">{cd.message.clone()}</div>
                                                    <div class="git-commit-meta-info flex items-center gap-3 text-xs text-text-muted">
                                                        <span>{cd.author.clone()}</span>
                                                        <span>{format_relative_time(&cd.date)}</span>
                                                    </div>
                                                    // Changed files summary + expand/collapse controls (inside meta)
                                                    <div class="git-commit-files-summary flex items-center justify-between mt-2">
                                                        <div class="text-xs text-text-muted">
                                                            {format!("{} file{} changed", file_count, if file_count != 1 { "s" } else { "" })}
                                                        </div>
                                                        <div class="git-commit-expand-controls flex items-center gap-1">
                                                            <button
                                                                class="git-commit-expand-btn text-[10px] text-text-muted hover:text-text px-1.5 py-0.5 rounded hover:bg-bg-panel/50"
                                                                on:click=expand_all_files
                                                                title="Expand all files"
                                                            >
                                                                "Expand all"
                                                            </button>
                                                            <button
                                                                class="git-commit-expand-btn text-[10px] text-text-muted hover:text-text px-1.5 py-0.5 rounded hover:bg-bg-panel/50"
                                                                on:click=collapse_all_files
                                                                title="Collapse all files"
                                                            >
                                                                "Collapse all"
                                                            </button>
                                                        </div>
                                                    </div>
                                                </div>
                                                // Per-file diff accordions
                                                <div class="git-commit-file-accordions">
                                                    {cd.files.iter().enumerate().map(|(_i, f)| {
                                                        let file_path = f.path.clone();
                                                        let file_path_click = f.path.clone();
                                                        let status_cls = match f.status.as_str() {
                                                            "A" | "added" => "text-success",
                                                            "D" | "deleted" => "text-error",
                                                            _ => "text-warning",
                                                        };
                                                        // Dir/filename split
                                                        let file_name = f.path.split('/').last().unwrap_or(&f.path).to_string();
                                                        let dir_path = if f.path.contains('/') {
                                                            f.path[..f.path.rfind('/').unwrap_or(0) + 1].to_string()
                                                        } else {
                                                            String::new()
                                                        };
                                                        // Find this file's diff
                                                        let file_diff: String = per_file_diffs.iter()
                                                            .find(|(p, _)| p == &f.path)
                                                            .map(|(_, d)| d.clone())
                                                            .unwrap_or_default();
                                                        let status_str = f.status.clone();
                                                        let file_path_class = f.path.clone();
                                                        view! {
                                                            <div class=move || {
                                                                if expanded_files.get().contains(&file_path_class) {
                                                                    "git-commit-file-accordion expanded mb-1"
                                                                } else {
                                                                    "git-commit-file-accordion mb-1"
                                                                }
                                                            }>
                                                                <button
                                                                    class="git-commit-file-header flex items-center gap-1.5 w-full text-left px-2 py-1 text-xs hover:bg-bg-element/40 rounded"
                                                                    on:click=move |_| toggle_file_accordion(file_path_click.clone())
                                                                >
                                                                    {
                                                                        let fp = file_path.clone();
                                                                        move || {
                                                                            if expanded_files.get().contains(&fp) {
                                                                                view! { <IconChevronDown size=12 /> }.into_any()
                                                                            } else {
                                                                                view! { <IconChevronRight size=12 /> }.into_any()
                                                                            }
                                                                        }
                                                                    }
                                                                    <span class=status_cls>{status_str.clone()}</span>
                                                                    <span class="git-commit-file-path text-text truncate">
                                                                        {if !dir_path.is_empty() {
                                                                            Some(view! { <span class="git-commit-file-dir text-text-muted">{dir_path.clone()}</span> })
                                                                        } else {
                                                                            None
                                                                        }}
                                                                        <span class="git-commit-file-name">{file_name.clone()}</span>
                                                                    </span>
                                                                </button>
                                                                {
                                                                    let fp2 = file_path.clone();
                                                                    let fd = file_diff.clone();
                                                                    move || {
                                                                        if expanded_files.get().contains(&fp2) {
                                                                            if fd.is_empty() {
                                                                                Some(view! {
                                                                                    <div class="git-commit-file-body git-empty git-empty-sm px-4 py-2 text-xs text-text-muted">"No diff available for this file"</div>
                                                                                }.into_any())
                                                                            } else {
                                                                                Some(view! {
                                                                                    <div class="git-commit-file-body">
                                                                                        <pre class="text-xs font-mono leading-5 overflow-x-auto whitespace-pre px-2 py-1 rounded bg-bg-element/30">
                                                                                            {fd.lines().map(|line| {
                                                                                                let cls = if line.starts_with('+') && !line.starts_with("+++") {
                                                                                                    "text-success bg-success/5"
                                                                                                } else if line.starts_with('-') && !line.starts_with("---") {
                                                                                                    "text-error bg-error/5"
                                                                                                } else if line.starts_with("@@") {
                                                                                                    "text-info"
                                                                                                } else {
                                                                                                    "text-text"
                                                                                                };
                                                                                                let line_str = format!("{}\n", line);
                                                                                                view! { <span class=cls>{line_str}</span> }
                                                                                            }).collect::<Vec<_>>()}
                                                                                        </pre>
                                                                                    </div>
                                                                                }.into_any())
                                                                            }
                                                                        } else {
                                                                            None
                                                                        }
                                                                    }
                                                                }
                                                            </div>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </div>
                                            </div>
                                        }.into_any()
                                    }
                                }
                            }
                        }
                        GitView::List => {
                            if active_tab.get() == "changes" {
                                render_changes_view(
                                    status,
                                    status_loading,
                                    commit_message,
                                    set_commit_message,
                                    committing,
                                    staged_open,
                                    set_staged_open,
                                    unstaged_open,
                                    set_unstaged_open,
                                    untracked_open,
                                    set_untracked_open,
                                    stage_file,
                                    unstage_file,
                                    discard_file,
                                    open_diff,
                                    do_commit,
                                    stage_all,
                                    unstage_all,
                                    on_send_to_ai,
                                    ai_review_loading,
                                    ai_commit_msg_loading,
                                    ai_pr_loading,
                                    handle_ai_review,
                                    handle_ai_commit_msg,
                                    open_pr_branch_picker,
                                )
                            } else {
                                render_log_view(log_entries, log_loading, open_commit)
                            }
                        }
                    }
                }}
            </div>

            // Error bar at bottom
            {move || {
                error_msg.get().map(|msg| {
                    view! {
                        <div class="git-error-bar flex items-center justify-between px-3 py-1.5 bg-error/10 border-t border-error/30 text-error text-xs">
                            <span>{msg}</span>
                            <button
                                class="text-error hover:text-text ml-2 flex-shrink-0"
                                on:click=move |_| set_error_msg.set(None)
                                title="Dismiss"
                            >
                                <IconX size=12 />
                            </button>
                        </div>
                    }
                })
            }}

            // PR Branch Picker overlay (matching React PRBranchPicker)
            {move || {
                if pr_branch_picker_open.get() {
                    let current_branch = branches.get()
                        .map(|b| b.current.clone())
                        .or_else(|| status.get().map(|s| s.branch.clone()))
                        .unwrap_or_default();
                    let br = branches.get();
                    let local = br.as_ref().map(|b| b.local.clone()).unwrap_or_default();
                    let remote = br.as_ref().map(|b| b.remote.clone()).unwrap_or_default();
                    let loading = branches_loading.get() || ai_pr_loading.get();
                    Some(view! {
                        <PRBranchPicker
                            current_branch=current_branch
                            local_branches=local
                            remote_branches=remote
                            loading=loading
                            on_select=Callback::new(move |base: String| handle_ai_pr_description(base))
                            on_close=Callback::new(move |_| set_pr_branch_picker_open.set(false))
                        />
                    })
                } else {
                    None
                }
            }}

            // PR Modal overlay (matching React PRModal)
            {move || {
                if pr_modal_open.get() {
                    if let Some(data) = pr_modal_data.get() {
                        Some(view! {
                            <PRModal
                                data=data
                                on_close=Callback::new(move |_| set_pr_modal_open.set(false))
                            />
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }}
        </div>
    }
}

// ── PR Branch Picker (matching React PRBranchPicker.tsx) ───────────

/// Priority order for suggesting a default base branch.
const DEFAULT_BASES: &[&str] = &["main", "master", "develop", "dev"];

#[component]
fn PRBranchPicker(
    current_branch: String,
    local_branches: Vec<String>,
    remote_branches: Vec<String>,
    loading: bool,
    on_select: Callback<String>,
    on_close: Callback<()>,
) -> impl IntoView {
    let (filter, set_filter) = signal(String::new());

    // Deduplicated, sorted list of all branches (strip origin/ prefix, exclude HEAD and current)
    let current = current_branch.clone();
    let all_branches = {
        let mut set = HashSet::new();
        let mut result = Vec::new();
        for b in local_branches.iter().chain(remote_branches.iter()) {
            let name = b.strip_prefix("origin/").unwrap_or(b).to_string();
            if name == "HEAD" || name == current {
                continue;
            }
            if set.insert(name.clone()) {
                result.push(name);
            }
        }
        result.sort();
        result
    };

    // Suggest a default base
    let suggested_base = {
        let mut suggested = all_branches.first().cloned().unwrap_or_default();
        for base in DEFAULT_BASES {
            if all_branches.iter().any(|b| b == base) {
                suggested = base.to_string();
                break;
            }
        }
        suggested
    };

    let all_branches_clone = all_branches.clone();
    let suggested_clone = suggested_base.clone();

    let filtered = Memo::new(move |_| {
        let f = filter.get().to_lowercase();
        if f.is_empty() {
            all_branches_clone.clone()
        } else {
            all_branches_clone.iter()
                .filter(|b| b.to_lowercase().contains(&f))
                .cloned()
                .collect()
        }
    });

    view! {
        <div class="pr-branch-picker-overlay"
            on:click=move |_| on_close.run(())
        >
            <div class="pr-branch-picker"
                on:click=move |e: leptos::ev::MouseEvent| e.stop_propagation()
            >
                <div class="pr-branch-picker-header">
                    <IconGitPullRequest size=14 />
                    <span>"Draft PR: " {current_branch.clone()} " " <IconChevronRight size=12 /> " select target branch"</span>
                    <button class="pr-branch-picker-close" on:click=move |_| on_close.run(())>
                        <IconX size=14 />
                    </button>
                </div>
                <div class="pr-branch-picker-search">
                    <IconSearch size=12 />
                    <input
                        type="text"
                        placeholder="Filter branches..."
                        prop:value=move || filter.get()
                        on:input=move |e| set_filter.set(event_target_value(&e))
                        on:keydown={
                            let suggested = suggested_clone.clone();
                            move |e: leptos::ev::KeyboardEvent| {
                                if e.key() == "Enter" {
                                    let f = filtered.get();
                                    if !f.is_empty() {
                                        // Select suggested if in list, otherwise first match
                                        let pick = if f.contains(&suggested) {
                                            suggested.clone()
                                        } else {
                                            f[0].clone()
                                        };
                                        on_select.run(pick);
                                    }
                                } else if e.key() == "Escape" {
                                    on_close.run(());
                                }
                            }
                        }
                    />
                </div>
                <div class="pr-branch-picker-list">
                    {move || {
                        let items = filtered.get();
                        if loading && items.is_empty() {
                            view! {
                                <div class="pr-branch-picker-empty">
                                    "Loading branches..."
                                </div>
                            }.into_any()
                        } else if items.is_empty() {
                            view! {
                                <div class="pr-branch-picker-empty">"No matching branches"</div>
                            }.into_any()
                        } else {
                            let suggested = suggested_base.clone();
                            view! {
                                <div>
                                    {items.iter().map(|b| {
                                        let branch = b.clone();
                                        let is_suggested = *b == suggested;
                                        let branch_click = b.clone();
                                        view! {
                                            <button
                                                class=if is_suggested { "pr-branch-picker-item suggested" } else { "pr-branch-picker-item" }
                                                on:click=move |_| on_select.run(branch_click.clone())
                                                disabled=loading
                                            >
                                                <span class="pr-branch-picker-item-name">{branch.clone()}</span>
                                                {if is_suggested {
                                                    Some(view! { <span class="pr-branch-picker-item-badge">"default"</span> })
                                                } else {
                                                    None
                                                }}
                                            </button>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            }.into_any()
                        }
                    }}
                </div>
            </div>
        </div>
    }
}

// ── PR Modal (matching React PRModal.tsx) ──────────────────────────

#[component]
fn PRModal(
    data: GitRangeDiffResponse,
    on_close: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="pr-description-modal-overlay"
            on:click=move |_| on_close.run(())
        >
            <div class="pr-description-modal"
                on:click=move |e: leptos::ev::MouseEvent| e.stop_propagation()
            >
                <div class="pr-description-modal-header">
                    <IconGitPullRequest size=16 />
                    <span>"PR Context: " {data.branch.clone()} " → " {data.base.clone()}</span>
                    <button class="pr-description-modal-close" on:click=move |_| on_close.run(())>
                        <IconX size=14 />
                    </button>
                </div>
                <div class="pr-description-modal-body">
                    <div class="pr-description-stats">
                        <span>{format!("{} commits", data.commits.len())}</span>
                        <span>{format!("{} files changed", data.files_changed)}</span>
                    </div>
                    <div class="pr-description-commits">
                        <h4>"Commits"</h4>
                        <ul>
                            {data.commits.iter().map(|c| {
                                let short = c.hash[..7.min(c.hash.len())].to_string();
                                let msg = c.message.clone();
                                view! {
                                    <li>
                                        <code>{short}</code>
                                        " " {msg}
                                    </li>
                                }
                            }).collect::<Vec<_>>()}
                        </ul>
                    </div>
                    <p class="pr-description-hint">
                        <IconSparkles size=12 />
                        " AI is generating a PR description in the chat panel..."
                    </p>
                </div>
            </div>
        </div>
    }
}

// ── Changes view ───────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_changes_view(
    status: ReadSignal<Option<GitStatusResponse>>,
    status_loading: ReadSignal<bool>,
    commit_message: ReadSignal<String>,
    set_commit_message: WriteSignal<String>,
    committing: ReadSignal<bool>,
    staged_open: ReadSignal<bool>,
    set_staged_open: WriteSignal<bool>,
    unstaged_open: ReadSignal<bool>,
    set_unstaged_open: WriteSignal<bool>,
    untracked_open: ReadSignal<bool>,
    set_untracked_open: WriteSignal<bool>,
    stage_file: impl Fn(String) + Copy + Send + Sync + 'static,
    unstage_file: impl Fn(String) + Copy + Send + Sync + 'static,
    discard_file: impl Fn(String) + Copy + Send + Sync + 'static,
    open_diff: impl Fn(String, bool) + Copy + Send + Sync + 'static,
    do_commit: impl Fn(()) + Copy + Send + Sync + 'static,
    stage_all: impl Fn(()) + Copy + Send + Sync + 'static,
    unstage_all: impl Fn(()) + Copy + Send + Sync + 'static,
    on_send_to_ai: Option<Callback<String>>,
    ai_review_loading: ReadSignal<bool>,
    ai_commit_msg_loading: ReadSignal<bool>,
    ai_pr_loading: ReadSignal<bool>,
    handle_ai_review: impl Fn(()) + Copy + Send + Sync + 'static,
    handle_ai_commit_msg: impl Fn(()) + Copy + Send + Sync + 'static,
    open_pr_branch_picker: impl Fn(()) + Copy + Send + Sync + 'static,
) -> leptos::prelude::AnyView {
    let has_send_to_ai = on_send_to_ai.is_some();
    view! {
        <div class="git-changes-list">
            {move || {
                if status_loading.get() {
                    return view! {
                        <div class="git-loading"><IconLoader2 size=18 /></div>
                    }.into_any();
                }

                match status.get() {
                    None => view! {
                        <div class="flex items-center justify-center py-8 text-text-muted text-sm">
                            "No repository selected"
                        </div>
                    }.into_any(),
                    Some(s) => {
                        let has_staged = !s.staged.is_empty();
                        let has_unstaged = !s.unstaged.is_empty();
                        let has_untracked = !s.untracked.is_empty();
                        let is_clean = !has_staged && !has_unstaged && !has_untracked;
                        let staged_count = s.staged.len();

                        if is_clean {
                            return view! {
                                <div class="git-empty flex items-center justify-center py-8 text-text-muted text-sm">
                                    <div class="text-center">
                                        <div class="text-success text-lg mb-1"><IconCheck size=20 /></div>
                                        <div>"Working tree clean"</div>
                                    </div>
                                </div>
                            }.into_any();
                        }

                        view! {
                            <div class="px-2 py-1">
                                // AI action buttons (matching React ChangesListView git-ai-actions)
                                {if has_send_to_ai {
                                    let has_any_changes = has_staged || has_unstaged;
                                    Some(view! {
                                        <div class="git-ai-actions">
                                            <button
                                                class="git-ai-button"
                                                on:click=move |_| handle_ai_review(())
                                                disabled=move || ai_review_loading.get() || !has_any_changes
                                                title="Send all changes to AI for code review"
                                            >
                                                {move || if ai_review_loading.get() {
                                                    view! { <IconLoader2 size=12 class="spin" /> }.into_any()
                                                } else {
                                                    view! { <IconMessageSquare size=12 /> }.into_any()
                                                }}
                                                " Review Changes"
                                            </button>
                                            <button
                                                class="git-ai-button"
                                                on:click=move |_| handle_ai_commit_msg(())
                                                disabled=move || ai_commit_msg_loading.get() || !has_staged
                                                title="Generate a commit message from staged changes"
                                            >
                                                {move || if ai_commit_msg_loading.get() {
                                                    view! { <IconLoader2 size=12 class="spin" /> }.into_any()
                                                } else {
                                                    view! { <IconFileEdit size=12 /> }.into_any()
                                                }}
                                                " Write Commit Msg"
                                            </button>
                                            <button
                                                class="git-ai-button"
                                                on:click=move |_| open_pr_branch_picker(())
                                                disabled=move || ai_pr_loading.get()
                                                title="Draft a PR description from branch changes"
                                            >
                                                {move || if ai_pr_loading.get() {
                                                    view! { <IconLoader2 size=12 class="spin" /> }.into_any()
                                                } else {
                                                    view! { <IconGitPullRequest size=12 /> }.into_any()
                                                }}
                                                " Draft PR"
                                            </button>
                                        </div>
                                    })
                                } else {
                                    None
                                }}

                                // Commit form ABOVE sections (matching React)
                                {if has_staged {
                                    Some(view! {
                                        <div class="git-commit-form mb-3 px-1 pb-2 border-b border-border-subtle">
                                            <textarea
                                                class="git-commit-input w-full bg-background text-text text-xs px-2 py-1.5 rounded border border-border-subtle focus:border-primary outline-none resize-none"
                                                rows=3
                                                placeholder="Commit message..."
                                                prop:value=move || commit_message.get()
                                                on:input=move |e| set_commit_message.set(event_target_value(&e))
                                                on:keydown=move |e: leptos::ev::KeyboardEvent| {
                                                    if (e.meta_key() || e.ctrl_key()) && e.key() == "Enter" {
                                                        e.prevent_default();
                                                        do_commit(());
                                                    }
                                                }
                                            />
                                            <button
                                                class="git-commit-button mt-1 w-full text-xs py-1.5 rounded bg-primary text-white hover:bg-primary/80 transition-colors disabled:opacity-50"
                                                disabled=move || committing.get() || commit_message.get().trim().is_empty()
                                                on:click=move |_| do_commit(())
                                                            >
                                                                {move || if committing.get() {
                                                                    view! { <IconLoader2 size=13 class="spin" /> }.into_any()
                                                                } else {
                                                                    view! { <IconGitCommitHorizontal size=13 /> }.into_any()
                                                                }}
                                                                {move || if committing.get() {
                                                                    " Committing...".to_string()
                                                                } else {
                                                                    let count = status.get().map(|s| s.staged.len()).unwrap_or(0);
                                                                    format!(" Commit ({} staged)", count)
                                                                }}
                                            </button>
                                        </div>
                                    })
                                } else {
                                    None
                                }}

                                // Staged section
                                {if has_staged {
                                    Some(view! {
                                        <div class="git-section mb-2">
                                            <button
                                                class="git-section-header flex items-center gap-1 w-full text-left px-1 py-0.5 text-xs text-success font-medium hover:bg-bg-element/30 rounded"
                                                on:click=move |_| set_staged_open.update(|v| *v = !*v)
                                            >
                                                {move || if staged_open.get() {
                                                    view! { <IconChevronDown size=12 /> }.into_any()
                                                } else {
                                                    view! { <IconChevronRight size=12 /> }.into_any()
                                                }}
                                                <span class="git-section-title flex-1">{format!("Staged ({})", staged_count)}</span>
                                                <button
                                                    class="git-section-action text-text-muted hover:text-warning text-[10px] px-1"
                                                    title="Unstage all"
                                                    on:click=move |e: leptos::ev::MouseEvent| {
                                                        e.stop_propagation();
                                                        unstage_all(());
                                                    }
                                                >
                                                    <IconMinus size=11 />
                                                </button>
                                            </button>
                                            {move || {
                                                if !staged_open.get() {
                                                    return None;
                                                }
                                                status.get().map(|s| {
                                                    s.staged.iter().map(|entry| {
                                                        let path = entry.path.clone();
                                                        let path_diff = entry.path.clone();
                                                        let path_diff_kb = entry.path.clone();
                                                        let path_unstage = entry.path.clone();
                                                        let status_str = entry.status.clone();
                                                        view! {
                                                            <div class="git-file-row flex items-center gap-1.5 px-2 py-0.5 text-xs hover:bg-bg-element/40 rounded group cursor-pointer"
                                                                role="button"
                                                                tabindex="0"
                                                                on:click=move |_| open_diff(path_diff.clone(), true)
                                                                on:keydown=move |e: leptos::ev::KeyboardEvent| {
                                                                    if e.key() == "Enter" || e.key() == " " {
                                                                        e.prevent_default();
                                                                        open_diff(path_diff_kb.clone(), true);
                                                                    }
                                                                }
                                                            >
                                                                <span
                                                                    class="git-file-status text-[10px] w-3 text-center"
                                                                    style=format!("color: {}", status_color(&status_str))
                                                                    title=status_label(&status_str)
                                                                >{status_str.clone()}</span>
                                                                <span class="git-file-path text-text truncate flex-1">{path.clone()}</span>
                                                                <button
                                                                    class="git-file-action opacity-0 group-hover:opacity-100 text-text-muted hover:text-warning text-[10px]"
                                                                    title="Unstage"
                                                                    on:click=move |e: leptos::ev::MouseEvent| {
                                                                        e.stop_propagation();
                                                                        unstage_file(path_unstage.clone());
                                                                    }
                                                                >
                                                                    <IconMinus size=11 />
                                                                </button>
                                                            </div>
                                                        }
                                                    }).collect::<Vec<_>>()
                                                })
                                            }}
                                        </div>
                                    })
                                } else {
                                    None
                                }}

                                // Unstaged (Modified) section
                                {if has_unstaged {
                                    let unstaged_count = s.unstaged.len();
                                    Some(view! {
                                        <div class="git-section mb-2">
                                            <button
                                                class="git-section-header flex items-center gap-1 w-full text-left px-1 py-0.5 text-xs text-warning font-medium hover:bg-bg-element/30 rounded"
                                                on:click=move |_| set_unstaged_open.update(|v| *v = !*v)
                                            >
                                                {move || if unstaged_open.get() {
                                                    view! { <IconChevronDown size=12 /> }.into_any()
                                                } else {
                                                    view! { <IconChevronRight size=12 /> }.into_any()
                                                }}
                                                <span class="git-section-title flex-1">{format!("Changes ({})", unstaged_count)}</span>
                                                <button
                                                    class="git-section-action text-text-muted hover:text-success text-[10px] px-1"
                                                    title="Stage all"
                                                    on:click=move |e: leptos::ev::MouseEvent| {
                                                        e.stop_propagation();
                                                        stage_all(());
                                                    }
                                                >
                                                    <IconPlus size=11 />
                                                </button>
                                            </button>
                                            {move || {
                                                if !unstaged_open.get() {
                                                    return None;
                                                }
                                                status.get().map(|s| {
                                                    s.unstaged.iter().map(|entry| {
                                                        let path = entry.path.clone();
                                                        let path_diff = entry.path.clone();
                                                        let path_diff_kb = entry.path.clone();
                                                        let path_stage = entry.path.clone();
                                                        let path_discard = entry.path.clone();
                                                        let status_str = entry.status.clone();
                                                        view! {
                                                            <div class="git-file-row flex items-center gap-1.5 px-2 py-0.5 text-xs hover:bg-bg-element/40 rounded group cursor-pointer"
                                                                role="button"
                                                                tabindex="0"
                                                                on:click=move |_| open_diff(path_diff.clone(), false)
                                                                on:keydown=move |e: leptos::ev::KeyboardEvent| {
                                                                    if e.key() == "Enter" || e.key() == " " {
                                                                        e.prevent_default();
                                                                        open_diff(path_diff_kb.clone(), false);
                                                                    }
                                                                }
                                                            >
                                                                <span
                                                                    class="git-file-status text-[10px] w-3 text-center"
                                                                    style=format!("color: {}", status_color(&status_str))
                                                                    title=status_label(&status_str)
                                                                >{status_str.clone()}</span>
                                                                <span class="git-file-path text-text truncate flex-1">{path.clone()}</span>
                                                                <div class="git-file-actions opacity-0 group-hover:opacity-100 flex items-center gap-0.5">
                                                                    <button
                                                                        class="git-file-action text-text-muted hover:text-error text-[10px]"
                                                                        title="Discard"
                                                                        on:click=move |e: leptos::ev::MouseEvent| {
                                                                            e.stop_propagation();
                                                                            discard_file(path_discard.clone());
                                                                        }
                                                                    >
                                                                        <IconTrash2 size=11 />
                                                                    </button>
                                                                    <button
                                                                        class="git-file-action text-text-muted hover:text-success text-[10px]"
                                                                        title="Stage"
                                                                        on:click=move |e: leptos::ev::MouseEvent| {
                                                                            e.stop_propagation();
                                                                            stage_file(path_stage.clone());
                                                                        }
                                                                    >
                                                                        <IconPlus size=11 />
                                                                    </button>
                                                                </div>
                                                            </div>
                                                        }
                                                    }).collect::<Vec<_>>()
                                                })
                                            }}
                                        </div>
                                    })
                                } else {
                                    None
                                }}

                                // Untracked section
                                {if has_untracked {
                                    let untracked_count = s.untracked.len();
                                    Some(view! {
                                        <div class="git-section mb-2">
                                            <button
                                                class="git-section-header flex items-center gap-1 w-full text-left px-1 py-0.5 text-xs text-text-muted font-medium hover:bg-bg-element/30 rounded"
                                                on:click=move |_| set_untracked_open.update(|v| *v = !*v)
                                            >
                                                {move || if untracked_open.get() {
                                                    view! { <IconChevronDown size=12 /> }.into_any()
                                                } else {
                                                    view! { <IconChevronRight size=12 /> }.into_any()
                                                }}
                                                <span class="git-section-title flex-1">{format!("Untracked ({})", untracked_count)}</span>
                                                <button
                                                    class="git-section-action text-text-muted hover:text-success text-[10px] px-1"
                                                    title="Stage all"
                                                    on:click=move |e: leptos::ev::MouseEvent| {
                                                        e.stop_propagation();
                                                        stage_all(());
                                                    }
                                                >
                                                    <IconPlus size=11 />
                                                </button>
                                            </button>
                                            {move || {
                                                if !untracked_open.get() {
                                                    return None;
                                                }
                                                status.get().map(|s| {
                                                    s.untracked.iter().map(|entry| {
                                                        let path = entry.path.clone();
                                                        let path_stage = entry.path.clone();
                                                        view! {
                                                            <div class="git-file-row flex items-center gap-1.5 px-2 py-0.5 text-xs hover:bg-bg-element/40 rounded">
                                                                <span class="git-file-status text-text-muted text-[10px] w-3 text-center" title="Untracked">"?"</span>
                                                                <span class="git-file-path text-text truncate flex-1">{path.clone()}</span>
                                                                <button
                                                                    class="git-file-action text-text-muted hover:text-success text-[10px]"
                                                                    title="Stage"
                                                                    on:click=move |e: leptos::ev::MouseEvent| {
                                                                        e.stop_propagation();
                                                                        stage_file(path_stage.clone());
                                                                    }
                                                                >
                                                                    <IconPlus size=11 />
                                                                </button>
                                                            </div>
                                                        }
                                                    }).collect::<Vec<_>>()
                                                })
                                            }}
                                        </div>
                                    })
                                } else {
                                    None
                                }}
                            </div>
                        }.into_any()
                    }
                }
            }}
        </div>
    }.into_any()
}

// ── Log view ───────────────────────────────────────────────────────

fn render_log_view(
    log_entries: ReadSignal<Vec<GitLogEntry>>,
    log_loading: ReadSignal<bool>,
    open_commit: impl Fn(String, String) + Copy + Send + Sync + 'static,
) -> leptos::prelude::AnyView {
    view! {
        <div class="git-log-view">
            {move || {
                if log_loading.get() {
                    return view! {
                        <div class="git-loading"><IconLoader2 size=18 /></div>
                    }.into_any();
                }

                let entries = log_entries.get();
                if entries.is_empty() {
                    return view! {
                        <div class="flex items-center justify-center py-8 text-text-muted text-sm">
                            "No commits found"
                        </div>
                    }.into_any();
                }

                view! {
                    <div class="py-1">
                        {entries.iter().map(|entry| {
                            let hash = entry.hash.clone();
                            let short_hash = entry.short_hash.clone();
                            let author = entry.author.clone();
                            let date = entry.date.clone();
                            let message = entry.message.clone();
                            let hash_click = entry.hash.clone();
                            let short_click = entry.short_hash.clone();

                            view! {
                                <div
                                    class="git-log-entry git-log-entry-clickable flex items-start gap-2 px-3 py-1.5 text-xs hover:bg-bg-element/40 cursor-pointer border-b border-border-subtle/30"
                                    role="button"
                                    tabindex="0"
                                    on:click=move |_| open_commit(hash_click.clone(), short_click.clone())
                                    on:keydown={
                                        let hk = hash.clone();
                                        let sk = short_hash.clone();
                                        move |e: leptos::ev::KeyboardEvent| {
                                            if e.key() == "Enter" || e.key() == " " {
                                                e.prevent_default();
                                                open_commit(hk.clone(), sk.clone());
                                            }
                                        }
                                    }
                                >
                                    <div class="flex-shrink-0 mt-0.5">
                                        <span class="git-log-hash text-primary font-mono text-[10px]">{short_hash.clone()}</span>
                                    </div>
                                    <div class="flex-1 min-w-0">
                                        <div class="git-log-message text-text truncate">{message.clone()}</div>
                                        <div class="git-log-meta flex items-center gap-2 text-text-muted text-[10px] mt-0.5">
                                            <span>{author.clone()}</span>
                                            <span>{format_relative_time(&date)}</span>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
        </div>
    }.into_any()
}
