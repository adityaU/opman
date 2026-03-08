/**
 * GitPanel — web-native Git UI replacing the gitui PTY panel.
 *
 * Features:
 * - Mobile-first layout (vertical stack, no side-by-side splits)
 * - Breadcrumb navigation with back button + dropdown
 * - File status list (staged, unstaged, untracked) with stage/unstage actions
 * - Commit form at the TOP of the file list (when staged files exist)
 * - Inline diff viewer for selected files (theme-aware colors)
 * - Commit log with click-to-view full commit diffs
 */
import { useState, useEffect, useCallback, useRef } from "react";
import ReactDiffViewer, { DiffMethod } from "react-diff-viewer-continued";
import {
  GitBranch,
  Plus,
  Minus,
  Check,
  X,
  FileText,
  RefreshCw,
  ChevronDown,
  ChevronRight,
  ChevronLeft,
  Trash2,
  Loader2,
  GitCommitHorizontal,
  History,
  Sparkles,
  MessageSquare,
  FileEdit,
  GitPullRequest,
} from "lucide-react";
import {
  fetchGitStatus,
  fetchGitDiff,
  fetchGitLog,
  fetchGitShow,
  fetchGitBranches,
  fetchGitRangeDiff,
  fetchGitContextSummary,
  gitCheckout,
  gitStage,
  gitUnstage,
  gitCommit,
  gitDiscard,
  fetchTheme,
  type GitFileEntry,
  type GitLogEntry,
  type GitShowResponse,
  type ThemeColors,
} from "./api";

// ── Navigation views ────────────────────────────────────

type GitTab = "changes" | "log";

/**
 * View stack for breadcrumb navigation:
 * - "list"       — file list (Changes or Log tab)
 * - "file-diff"  — viewing diff for a single working-tree file
 * - "commit"     — viewing full commit diff from log
 */
type GitView =
  | { kind: "list" }
  | { kind: "file-diff"; file: string; staged: boolean }
  | { kind: "commit"; hash: string; shortHash: string };

// ── Status icon helpers ─────────────────────────────────

function statusLabel(status: string): string {
  switch (status) {
    case "M":
      return "Modified";
    case "A":
      return "Added";
    case "D":
      return "Deleted";
    case "R":
      return "Renamed";
    case "?":
      return "Untracked";
    case "U":
      return "Unmerged";
    default:
      return status;
  }
}

function statusColor(status: string): string {
  switch (status) {
    case "M":
      return "var(--color-warning)";
    case "A":
      return "var(--color-success)";
    case "D":
      return "var(--color-error)";
    case "?":
      return "var(--color-text-muted)";
    default:
      return "var(--color-text)";
  }
}

// ── Diff theme builder ──────────────────────────────────

function buildDiffStyles(theme: ThemeColors | null) {
  const css = typeof window !== "undefined" ? getComputedStyle(document.documentElement) : null;
  const success = theme?.success || css?.getPropertyValue("--color-success").trim() || "#7fd88f";
  const error = theme?.error || css?.getPropertyValue("--color-error").trim() || "#e06c75";
  const textMuted = theme?.text_muted || css?.getPropertyValue("--color-text-muted").trim() || "#808080";

  return {
    variables: {
      dark: {
        diffViewerBackground: "transparent",
        gutterBackground: "transparent",
        addedBackground: hexToRgba(success, 0.12),
        addedColor: success,
        removedBackground: hexToRgba(error, 0.12),
        removedColor: error,
        wordAddedBackground: hexToRgba(success, 0.25),
        wordRemovedBackground: hexToRgba(error, 0.25),
        addedGutterBackground: hexToRgba(success, 0.08),
        removedGutterBackground: hexToRgba(error, 0.08),
        gutterColor: textMuted,
        codeFoldGutterBackground: "transparent",
        codeFoldBackground: "var(--theme-surface-3, var(--color-bg-element))",
        emptyLineBackground: "transparent",
        codeFoldContentColor: textMuted,
      },
    },
    contentText: {
      fontFamily: "var(--font-mono, monospace)",
      fontSize: "12px",
      lineHeight: "1.5",
    },
  };
}

function hexToRgba(hex: string, alpha: number): string {
  const h = hex.replace("#", "");
  const r = parseInt(h.substring(0, 2), 16) || 0;
  const g = parseInt(h.substring(2, 4), 16) || 0;
  const b = parseInt(h.substring(4, 6), 16) || 0;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

// ── Component ───────────────────────────────────────────

interface Props {
  focused?: boolean;
  /** Project path — when this changes, the git panel resets and re-fetches */
  projectPath?: string | null;
  /** Callback for surfacing errors to the user (e.g. toast) */
  onError?: (message: string) => void;
  /** Send a message to the AI chat (used by AI context buttons) */
  onSendToAI?: (text: string) => void;
  /** Callback to populate the commit message textarea (for AI-generated commit messages) */
  onCommitMessageGenerated?: (message: string) => void;
}

export default function GitPanel({ focused: _focused, projectPath, onError, onSendToAI }: Props) {
  const [tab, setTab] = useState<GitTab>("changes");
  const [viewStack, setViewStack] = useState<GitView[]>([{ kind: "list" }]);
  const [breadcrumbDropdown, setBreadcrumbDropdown] = useState(false);

  // Theme colors for diff viewer
  const [themeColors, setThemeColors] = useState<ThemeColors | null>(null);

  // Status state
  const [branch, setBranch] = useState("");
  const [staged, setStaged] = useState<GitFileEntry[]>([]);
  const [unstaged, setUnstaged] = useState<GitFileEntry[]>([]);
  const [untracked, setUntracked] = useState<GitFileEntry[]>([]);
  const [loading, setLoading] = useState(false);

  // Section collapse state
  const [stagedOpen, setStagedOpen] = useState(true);
  const [unstagedOpen, setUnstagedOpen] = useState(true);
  const [untrackedOpen, setUntrackedOpen] = useState(true);

  // Diff state (for working-tree file diffs)
  const [diffOld, setDiffOld] = useState("");
  const [diffNew, setDiffNew] = useState("");
  const [diffLoading, setDiffLoading] = useState(false);

  // Commit state
  const [commitMsg, setCommitMsg] = useState("");
  const [committing, setCommitting] = useState(false);

  // Log state
  const [commits, setCommits] = useState<GitLogEntry[]>([]);
  const [logLoading, setLogLoading] = useState(false);

  // Commit show state (for viewing commit diffs from log)
  const [commitDetail, setCommitDetail] = useState<GitShowResponse | null>(
    null
  );
  const [commitDetailLoading, setCommitDetailLoading] = useState(false);

  // Commit detail per-file accordion state
  const [expandedFiles, setExpandedFiles] = useState<Set<string>>(new Set());

  const commitInputRef = useRef<HTMLTextAreaElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const branchDropdownRef = useRef<HTMLDivElement>(null);

  // Branch switcher state
  const [showBranchDropdown, setShowBranchDropdown] = useState(false);
  const [localBranches, setLocalBranches] = useState<string[]>([]);
  const [remoteBranches, setRemoteBranches] = useState<string[]>([]);
  const [branchesLoading, setBranchesLoading] = useState(false);
  const [checkingOut, setCheckingOut] = useState(false);
  const [branchFilter, setBranchFilter] = useState("");

  // AI context action state
  const [aiReviewLoading, setAiReviewLoading] = useState(false);
  const [aiCommitMsgLoading, setAiCommitMsgLoading] = useState(false);
  const [aiPrLoading, setAiPrLoading] = useState(false);
  const [prModalOpen, setPrModalOpen] = useState(false);
  const [prModalData, setPrModalData] = useState<{
    branch: string;
    base: string;
    commits: GitLogEntry[];
    diff: string;
    files_changed: number;
  } | null>(null);

  // Current view is top of stack
  const currentView = viewStack[viewStack.length - 1];

  // ── Per-file accordion helpers ──────────────────────────

  const toggleFileAccordion = useCallback((filePath: string) => {
    setExpandedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(filePath)) {
        next.delete(filePath);
      } else {
        next.add(filePath);
      }
      return next;
    });
  }, []);

  const expandAllFiles = useCallback(() => {
    if (commitDetail?.files) {
      setExpandedFiles(new Set(commitDetail.files.map((f) => f.path)));
    }
  }, [commitDetail]);

  const collapseAllFiles = useCallback(() => {
    setExpandedFiles(new Set());
  }, []);

  // ── Navigation helpers ──────────────────────────────────

  const pushView = useCallback((view: GitView) => {
    setViewStack((s) => [...s, view]);
    setBreadcrumbDropdown(false);
  }, []);

  const popView = useCallback(() => {
    setViewStack((s) => (s.length > 1 ? s.slice(0, -1) : s));
    setBreadcrumbDropdown(false);
  }, []);

  const jumpToView = useCallback((index: number) => {
    setViewStack((s) => s.slice(0, index + 1));
    setBreadcrumbDropdown(false);
  }, []);

  // Close dropdown on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(e.target as Node)
      ) {
        setBreadcrumbDropdown(false);
      }
    };
    if (breadcrumbDropdown) {
      document.addEventListener("mousedown", handler);
      return () => document.removeEventListener("mousedown", handler);
    }
  }, [breadcrumbDropdown]);

  // Close branch dropdown on outside click
  useEffect(() => {
    if (!showBranchDropdown) return;
    const handler = (e: MouseEvent) => {
      if (
        branchDropdownRef.current &&
        !branchDropdownRef.current.contains(e.target as Node)
      ) {
        setShowBranchDropdown(false);
        setBranchFilter("");
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showBranchDropdown]);

  // Fetch branches when dropdown opens
  // ── Data fetching ───────────────────────────────────────

  // Load theme once
  useEffect(() => {
    fetchTheme().then((t) => {
      if (t) setThemeColors(t);
    });
  }, []);

  // Refresh status
  const refreshStatus = useCallback(async () => {
    setLoading(true);
    try {
      const status = await fetchGitStatus();
      setBranch(status.branch);
      setStaged(status.staged);
      setUnstaged(status.unstaged);
      setUntracked(status.untracked);
    } catch (err) {
      console.error("Failed to fetch git status:", err);
      onError?.("Failed to fetch git status");
    } finally {
      setLoading(false);
    }
  }, [onError]);

  // ── Branch switcher ─────────────────────────────────────

  const fetchBranchList = useCallback(async () => {
    setBranchesLoading(true);
    try {
      const data = await fetchGitBranches();
      setLocalBranches(data.local);
      setRemoteBranches(data.remote);
    } catch {
      onError?.("Failed to fetch branches");
    } finally {
      setBranchesLoading(false);
    }
  }, [onError]);

  const handleBranchToggle = useCallback(() => {
    setShowBranchDropdown((v) => {
      if (!v) {
        fetchBranchList();
        setBranchFilter("");
      }
      return !v;
    });
  }, [fetchBranchList]);

  const handleCheckout = useCallback(
    async (branchName: string) => {
      if (branchName === branch) {
        setShowBranchDropdown(false);
        return;
      }
      setCheckingOut(true);
      try {
        const result = await gitCheckout(branchName);
        if (result.success) {
          setBranch(branchName);
          setShowBranchDropdown(false);
          setBranchFilter("");
          // Refresh status after branch switch
          refreshStatus();
        } else {
          onError?.(result.message || "Checkout failed");
        }
      } catch {
        onError?.("Failed to checkout branch");
      } finally {
        setCheckingOut(false);
      }
    },
    [branch, onError, refreshStatus]
  );

  // Refresh log
  const refreshLog = useCallback(async () => {
    setLogLoading(true);
    try {
      const resp = await fetchGitLog(50);
      setCommits(resp.commits);
    } catch (err) {
      console.error("Failed to fetch git log:", err);
      onError?.("Failed to fetch git log");
    } finally {
      setLogLoading(false);
    }
  }, [onError]);

  // Initial load
  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  // Reset git panel state when project changes (session switch)
  const prevProjectPath = useRef(projectPath);
  useEffect(() => {
    if (projectPath === prevProjectPath.current) return;
    prevProjectPath.current = projectPath;
    // Reset data state (preserve UI prefs like tab, section collapse)
    setBranch("");
    setStaged([]);
    setUnstaged([]);
    setUntracked([]);
    setCommits([]);
    setCommitDetail(null);
    setCommitMsg("");
    setDiffOld("");
    setDiffNew("");
    setExpandedFiles(new Set());
    setViewStack([{ kind: "list" }]);
    // Reload git status for new project
    refreshStatus();
    // If on log tab, also refresh log
    if (tab === "log") refreshLog();
  }, [projectPath, refreshStatus, refreshLog, tab]);

  // Load log when switching to log tab
  useEffect(() => {
    if (tab === "log" && commits.length === 0) {
      refreshLog();
    }
  }, [tab, commits.length, refreshLog]);

  // Load diff when navigating to file-diff view
  useEffect(() => {
    if (currentView.kind === "file-diff") {
      const { file, staged: isStaged } = currentView;
      setDiffLoading(true);
      fetchGitDiff(file, isStaged)
        .then((resp) => {
          const { oldText, newText } = parseUnifiedDiff(resp.diff);
          setDiffOld(oldText);
          setDiffNew(newText);
        })
        .catch((err) => {
          console.error("Failed to fetch diff:", err);
          onError?.("Failed to load diff");
          setDiffOld("");
          setDiffNew("");
        })
        .finally(() => setDiffLoading(false));
    }
  }, [currentView]);

  // Load commit detail when navigating to commit view
  useEffect(() => {
    if (currentView.kind === "commit") {
      setCommitDetailLoading(true);
      setCommitDetail(null);
      setExpandedFiles(new Set());
      fetchGitShow(currentView.hash)
        .then((resp) => setCommitDetail(resp))
        .catch((err) => {
          console.error("Failed to fetch commit detail:", err);
          onError?.("Failed to load commit details");
        })
        .finally(() => setCommitDetailLoading(false));
    }
  }, [currentView]);

  // ── Actions ─────────────────────────────────────────────

  const handleStage = useCallback(
    async (files: string[]) => {
      try {
        await gitStage(files);
        await refreshStatus();
      } catch (err) {
        console.error("Failed to stage:", err);
        onError?.("Failed to stage file");
      }
    },
    [refreshStatus, onError]
  );

  const handleUnstage = useCallback(
    async (files: string[]) => {
      try {
        await gitUnstage(files);
        await refreshStatus();
      } catch (err) {
        console.error("Failed to unstage:", err);
        onError?.("Failed to unstage file");
      }
    },
    [refreshStatus, onError]
  );

  const handleStageAll = useCallback(async () => {
    try {
      await gitStage([]);
      await refreshStatus();
    } catch (err) {
      console.error("Failed to stage all:", err);
      onError?.("Failed to stage all files");
    }
  }, [refreshStatus, onError]);

  const handleUnstageAll = useCallback(async () => {
    try {
      await gitUnstage([]);
      await refreshStatus();
    } catch (err) {
      console.error("Failed to unstage all:", err);
      onError?.("Failed to unstage all files");
    }
  }, [refreshStatus, onError]);

  const handleDiscard = useCallback(
    async (files: string[]) => {
      if (
        !window.confirm(
          `Discard changes to ${files.length} file(s)? This cannot be undone.`
        )
      )
        return;
      try {
        await gitDiscard(files);
        await refreshStatus();
      } catch (err) {
        console.error("Failed to discard:", err);
        onError?.("Failed to discard changes");
      }
    },
    [refreshStatus, onError]
  );

  const handleCommit = useCallback(async () => {
    if (!commitMsg.trim()) return;
    setCommitting(true);
    try {
      await gitCommit(commitMsg);
      setCommitMsg("");
      await refreshStatus();
    } catch (err) {
      console.error("Failed to commit:", err);
      onError?.("Commit failed");
    } finally {
      setCommitting(false);
    }
  }, [commitMsg, refreshStatus, onError]);

  const handleCommitKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      handleCommit();
    }
  };

  // ── AI Context action handlers ────────────────────────────

  /** "Review My Changes" — gathers all diffs and sends to AI for review */
  const handleAIReview = useCallback(async () => {
    if (!onSendToAI) return;
    setAiReviewLoading(true);
    try {
      // Gather both staged and unstaged diffs
      const [stagedDiff, unstagedDiff] = await Promise.all([
        staged.length > 0 ? fetchGitDiff(undefined, true) : Promise.resolve({ diff: "" }),
        unstaged.length > 0 ? fetchGitDiff(undefined, false) : Promise.resolve({ diff: "" }),
      ]);
      const combinedDiff = [stagedDiff.diff, unstagedDiff.diff].filter(Boolean).join("\n");
      if (!combinedDiff.trim()) {
        onError?.("No changes to review");
        return;
      }
      const prompt = `Please review my current git changes. Here is the diff:\n\n\`\`\`diff\n${combinedDiff}\n\`\`\`\n\nProvide a thorough code review covering:\n- Potential bugs or issues\n- Code quality and style\n- Suggestions for improvement\n- Any security concerns`;
      onSendToAI(prompt);
    } catch (err) {
      console.error("Failed to gather diff for AI review:", err);
      onError?.("Failed to gather changes for review");
    } finally {
      setAiReviewLoading(false);
    }
  }, [onSendToAI, staged.length, unstaged.length, onError]);

  /** "Write Commit Message" — generates a commit message from staged changes */
  const handleAICommitMsg = useCallback(async () => {
    if (!onSendToAI) return;
    if (staged.length === 0) {
      onError?.("Stage some files first");
      return;
    }
    setAiCommitMsgLoading(true);
    try {
      const { diff } = await fetchGitDiff(undefined, true);
      if (!diff.trim()) {
        onError?.("No staged changes found");
        return;
      }
      const fileList = staged.map((f) => `  ${f.status} ${f.path}`).join("\n");
      const prompt = `Generate a concise, well-structured git commit message for the following staged changes.\n\nStaged files:\n${fileList}\n\nDiff:\n\`\`\`diff\n${diff}\n\`\`\`\n\nWrite a commit message following conventional commit format (e.g. feat:, fix:, refactor:). Include a brief subject line (max 72 chars) and an optional body with bullet points if needed. Return ONLY the commit message, nothing else.`;
      onSendToAI(prompt);
    } catch (err) {
      console.error("Failed to gather diff for commit message:", err);
      onError?.("Failed to gather staged changes");
    } finally {
      setAiCommitMsgLoading(false);
    }
  }, [onSendToAI, staged, onError]);

  /** "Draft PR Description" — gathers range diff and shows in modal + sends to AI */
  const handleAIPRDescription = useCallback(async () => {
    if (!onSendToAI) return;
    setAiPrLoading(true);
    try {
      const rangeData = await fetchGitRangeDiff();
      if (!rangeData.diff.trim() && rangeData.commits.length === 0) {
        onError?.("No commits found relative to base branch");
        return;
      }
      // Store data for the modal
      setPrModalData(rangeData);
      setPrModalOpen(true);
      // Build prompt
      const commitList = rangeData.commits
        .map((c) => `  - ${c.hash.slice(0, 7)} ${c.message}`)
        .join("\n");
      const prompt = `Draft a pull request description for merging \`${rangeData.branch}\` into \`${rangeData.base}\`.\n\nCommits (${rangeData.commits.length}):\n${commitList}\n\nFiles changed: ${rangeData.files_changed}\n\nDiff:\n\`\`\`diff\n${rangeData.diff.slice(0, 8000)}${rangeData.diff.length > 8000 ? "\n... (diff truncated)" : ""}\n\`\`\`\n\nWrite a clear PR description with:\n- A concise title\n- A summary section with bullet points\n- Any notable changes or breaking changes\n- Testing notes if applicable`;
      onSendToAI(prompt);
    } catch (err) {
      console.error("Failed to gather range diff for PR description:", err);
      onError?.("Failed to gather branch changes");
    } finally {
      setAiPrLoading(false);
    }
  }, [onSendToAI, onError]);

  const totalChanges = staged.length + unstaged.length + untracked.length;

  // ── Breadcrumb labels ───────────────────────────────────

  function breadcrumbLabel(view: GitView): string {
    switch (view.kind) {
      case "list":
        return tab === "changes" ? "Changes" : "Log";
      case "file-diff":
        return view.file.split("/").pop() || view.file;
      case "commit":
        return view.shortHash;
    }
  }

  // ── Diff styles (memoized on theme) ─────────────────────

  const diffStyles = buildDiffStyles(themeColors);

  // ── Render ──────────────────────────────────────────────

  return (
    <div className="git-panel">
      {/* Toolbar: breadcrumbs + branch + refresh + tabs */}
      <div className="git-panel-header">
        <div className="git-panel-toolbar-row">
          {/* Back button with dropdown */}
          {viewStack.length > 1 && (
            <div className="git-breadcrumb-back" ref={dropdownRef}>
              <button
                className="git-back-btn"
                onClick={popView}
                title="Go back"
                aria-label="Go back"
              >
                <ChevronLeft size={14} />
              </button>
              {viewStack.length > 2 && (
                <button
                  className="git-back-dropdown-btn"
                  onClick={() => setBreadcrumbDropdown((v) => !v)}
                  title="Jump to..."
                  aria-label="Jump to previous view"
                >
                  <ChevronDown size={10} />
                </button>
              )}
              {breadcrumbDropdown && (
                <div className="git-breadcrumb-dropdown">
                  {viewStack.slice(0, -1).map((v, i) => (
                    <button
                      key={i}
                      className="git-breadcrumb-dropdown-item"
                      onClick={() => jumpToView(i)}
                    >
                      {breadcrumbLabel(v)}
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Breadcrumb trail */}
          <div className="git-breadcrumb-trail">
            {viewStack.map((v, i) => (
              <span key={i} className="git-breadcrumb-segment">
                {i > 0 && (
                  <span className="git-breadcrumb-sep">/</span>
                )}
                {i < viewStack.length - 1 ? (
                  <button
                    className="git-breadcrumb-link"
                    onClick={() => jumpToView(i)}
                  >
                    {breadcrumbLabel(v)}
                  </button>
                ) : (
                  <span className="git-breadcrumb-current">
                    {breadcrumbLabel(v)}
                  </span>
                )}
              </span>
            ))}
          </div>

          {/* Branch switcher + refresh (right side) */}
          <div className="git-panel-branch" ref={branchDropdownRef}>
            <button
              className="git-branch-toggle"
              onClick={handleBranchToggle}
              title="Switch branch"
              aria-label="Switch branch"
              disabled={checkingOut}
            >
              <GitBranch size={13} />
              <span>{checkingOut ? "Switching..." : branch || "..."}</span>
              <ChevronDown size={10} className={showBranchDropdown ? "rotated" : ""} />
            </button>

            {/* Branch dropdown */}
            {showBranchDropdown && (
              <div className="git-branch-dropdown">
                <input
                  className="git-branch-filter"
                  placeholder="Filter branches..."
                  value={branchFilter}
                  onChange={(e) => setBranchFilter(e.target.value)}
                  autoFocus
                />
                {branchesLoading ? (
                  <div className="git-branch-loading">
                    <Loader2 size={14} className="spin" />
                    <span>Loading...</span>
                  </div>
                ) : (
                  <div className="git-branch-list">
                    {/* Local branches */}
                    {localBranches
                      .filter((b) => !branchFilter || b.toLowerCase().includes(branchFilter.toLowerCase()))
                      .map((b) => (
                        <button
                          key={b}
                          className={`git-branch-item ${b === branch ? "current" : ""}`}
                          onClick={() => handleCheckout(b)}
                          disabled={checkingOut}
                        >
                          <span className="git-branch-item-name">{b}</span>
                          {b === branch && <Check size={12} />}
                        </button>
                      ))}
                    {/* Remote branches */}
                    {remoteBranches
                      .filter((b) => !branchFilter || b.toLowerCase().includes(branchFilter.toLowerCase()))
                      .length > 0 && (
                      <>
                        <div className="git-branch-section-label">Remote</div>
                        {remoteBranches
                          .filter((b) => !branchFilter || b.toLowerCase().includes(branchFilter.toLowerCase()))
                          .map((b) => (
                            <button
                              key={b}
                              className="git-branch-item remote"
                              onClick={() => handleCheckout(b)}
                              disabled={checkingOut}
                            >
                              <span className="git-branch-item-name">{b}</span>
                            </button>
                          ))}
                      </>
                    )}
                    {localBranches.filter((b) => !branchFilter || b.toLowerCase().includes(branchFilter.toLowerCase())).length === 0 &&
                     remoteBranches.filter((b) => !branchFilter || b.toLowerCase().includes(branchFilter.toLowerCase())).length === 0 && (
                      <div className="git-branch-empty">No matching branches</div>
                    )}
                  </div>
                )}
              </div>
            )}

            <button
              className="git-panel-refresh"
              onClick={() => {
                if (tab === "changes") refreshStatus();
                else refreshLog();
              }}
              title="Refresh"
              aria-label="Refresh"
              disabled={loading || logLoading}
            >
              <RefreshCw
                size={12}
                className={loading || logLoading ? "spin" : ""}
              />
            </button>
          </div>
        </div>

        {/* Tabs (only show when on list view) */}
        {currentView.kind === "list" && (
          <div className="git-panel-tabs">
            <button
              className={`git-tab ${tab === "changes" ? "active" : ""}`}
              onClick={() => setTab("changes")}
            >
              Changes
              {totalChanges > 0 && (
                <span className="git-tab-badge">{totalChanges}</span>
              )}
            </button>
            <button
              className={`git-tab ${tab === "log" ? "active" : ""}`}
              onClick={() => {
                setTab("log");
                // Reset view stack when switching tabs
                setViewStack([{ kind: "list" }]);
              }}
            >
              <History size={12} />
              Log
            </button>
          </div>
        )}
      </div>

      {/* ── LIST VIEW (Changes tab) ─────────────────────── */}
      {currentView.kind === "list" && tab === "changes" && (
        <div className="git-changes-list">
          {loading ? (
            <div className="git-loading">
              <Loader2 size={18} className="spin" />
            </div>
          ) : totalChanges === 0 ? (
            <div className="git-empty">
              <Check size={20} />
              <span>Working tree clean</span>
            </div>
          ) : (
            <>
              {/* AI Context action buttons */}
              {onSendToAI && (
                <div className="git-ai-actions">
                  <button
                    className="git-ai-button"
                    onClick={handleAIReview}
                    disabled={aiReviewLoading || (staged.length === 0 && unstaged.length === 0)}
                    title="Send all changes to AI for code review"
                  >
                    {aiReviewLoading ? <Loader2 size={12} className="spin" /> : <MessageSquare size={12} />}
                    Review Changes
                  </button>
                  <button
                    className="git-ai-button"
                    onClick={handleAICommitMsg}
                    disabled={aiCommitMsgLoading || staged.length === 0}
                    title="Generate a commit message from staged changes"
                  >
                    {aiCommitMsgLoading ? <Loader2 size={12} className="spin" /> : <FileEdit size={12} />}
                    Write Commit Msg
                  </button>
                  <button
                    className="git-ai-button"
                    onClick={handleAIPRDescription}
                    disabled={aiPrLoading}
                    title="Draft a PR description from branch changes"
                  >
                    {aiPrLoading ? <Loader2 size={12} className="spin" /> : <GitPullRequest size={12} />}
                    Draft PR
                  </button>
                </div>
              )}

              {/* Commit form at TOP when staged files exist */}
              {staged.length > 0 && (
                <div className="git-commit-form">
                  <textarea
                    ref={commitInputRef}
                    className="git-commit-input"
                    placeholder="Commit message..."
                    value={commitMsg}
                    onChange={(e) => setCommitMsg(e.target.value)}
                    onKeyDown={handleCommitKeyDown}
                    rows={3}
                  />
                  <button
                    className="git-commit-button"
                    onClick={handleCommit}
                    disabled={!commitMsg.trim() || committing}
                  >
                    {committing ? (
                      <Loader2 size={13} className="spin" />
                    ) : (
                      <GitCommitHorizontal size={13} />
                    )}
                    Commit ({staged.length} staged)
                  </button>
                </div>
              )}

              {/* Staged section */}
              {staged.length > 0 && (
                <div className="git-section">
                  <button
                    className="git-section-header"
                    onClick={() => setStagedOpen((v) => !v)}
                  >
                    {stagedOpen ? (
                      <ChevronDown size={12} />
                    ) : (
                      <ChevronRight size={12} />
                    )}
                    <span className="git-section-title">
                      Staged ({staged.length})
                    </span>
                    <button
                      className="git-section-action"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleUnstageAll();
                      }}
                      title="Unstage all"
                      aria-label="Unstage all files"
                    >
                      <Minus size={11} />
                    </button>
                  </button>
                  {stagedOpen &&
                    staged.map((file) => (
                      <div
                        key={`staged-${file.path}`}
                        className="git-file-row"
                        onClick={() =>
                          pushView({
                            kind: "file-diff",
                            file: file.path,
                            staged: true,
                          })
                        }
                        role="button"
                        tabIndex={0}
                        onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pushView({ kind: "file-diff", file: file.path, staged: true }); } }}
                      >
                        <span
                          className="git-file-status"
                          style={{ color: statusColor(file.status) }}
                          title={statusLabel(file.status)}
                        >
                          {file.status}
                        </span>
                        <span className="git-file-path">{file.path}</span>
                        <button
                          className="git-file-action"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleUnstage([file.path]);
                          }}
                          title="Unstage"
                          aria-label={`Unstage ${file.path}`}
                        >
                          <Minus size={11} />
                        </button>
                      </div>
                    ))}
                </div>
              )}

              {/* Unstaged section */}
              {unstaged.length > 0 && (
                <div className="git-section">
                  <button
                    className="git-section-header"
                    onClick={() => setUnstagedOpen((v) => !v)}
                  >
                    {unstagedOpen ? (
                      <ChevronDown size={12} />
                    ) : (
                      <ChevronRight size={12} />
                    )}
                    <span className="git-section-title">
                      Changes ({unstaged.length})
                    </span>
                    <button
                      className="git-section-action"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleStageAll();
                      }}
                      title="Stage all"
                      aria-label="Stage all files"
                    >
                      <Plus size={11} />
                    </button>
                  </button>
                  {unstagedOpen &&
                    unstaged.map((file) => (
                      <div
                        key={`unstaged-${file.path}`}
                        className="git-file-row"
                        onClick={() =>
                          pushView({
                            kind: "file-diff",
                            file: file.path,
                            staged: false,
                          })
                        }
                        role="button"
                        tabIndex={0}
                        onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pushView({ kind: "file-diff", file: file.path, staged: false }); } }}
                      >
                        <span
                          className="git-file-status"
                          style={{ color: statusColor(file.status) }}
                          title={statusLabel(file.status)}
                        >
                          {file.status}
                        </span>
                        <span className="git-file-path">{file.path}</span>
                        <div className="git-file-actions">
                          <button
                            className="git-file-action"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleDiscard([file.path]);
                            }}
                            title="Discard changes"
                            aria-label={`Discard changes to ${file.path}`}
                          >
                            <Trash2 size={11} />
                          </button>
                          <button
                            className="git-file-action"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleStage([file.path]);
                            }}
                            title="Stage"
                            aria-label={`Stage ${file.path}`}
                          >
                            <Plus size={11} />
                          </button>
                        </div>
                      </div>
                    ))}
                </div>
              )}

              {/* Untracked section */}
              {untracked.length > 0 && (
                <div className="git-section">
                  <button
                    className="git-section-header"
                    onClick={() => setUntrackedOpen((v) => !v)}
                  >
                    {untrackedOpen ? (
                      <ChevronDown size={12} />
                    ) : (
                      <ChevronRight size={12} />
                    )}
                    <span className="git-section-title">
                      Untracked ({untracked.length})
                    </span>
                    <button
                      className="git-section-action"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleStageAll();
                      }}
                      title="Stage all"
                      aria-label="Stage all untracked files"
                    >
                      <Plus size={11} />
                    </button>
                  </button>
                  {untrackedOpen &&
                    untracked.map((file) => (
                      <div
                        key={`untracked-${file.path}`}
                        className="git-file-row"
                      >
                        <span
                          className="git-file-status"
                          style={{ color: statusColor(file.status) }}
                        >
                          ?
                        </span>
                        <span className="git-file-path">{file.path}</span>
                        <button
                          className="git-file-action"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleStage([file.path]);
                          }}
                          title="Stage"
                          aria-label={`Stage ${file.path}`}
                        >
                          <Plus size={11} />
                        </button>
                      </div>
                    ))}
                </div>
              )}
            </>
          )}
        </div>
      )}

      {/* ── LIST VIEW (Log tab) ─────────────────────────── */}
      {currentView.kind === "list" && tab === "log" && (
        <div className="git-log-view">
          {logLoading ? (
            <div className="git-loading">
              <Loader2 size={18} className="spin" />
            </div>
          ) : commits.length === 0 ? (
            <div className="git-empty">
              <span>No commits found</span>
            </div>
          ) : (
            commits.map((c) => (
              <div
                key={c.hash}
                className="git-log-entry git-log-entry-clickable"
                onClick={() =>
                  pushView({
                    kind: "commit",
                    hash: c.hash,
                    shortHash: c.short_hash,
                  })
                }
                role="button"
                tabIndex={0}
                onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pushView({ kind: "commit", hash: c.hash, shortHash: c.short_hash }); } }}
              >
                <div className="git-log-hash">{c.short_hash}</div>
                <div className="git-log-message">{c.message}</div>
                <div className="git-log-meta">
                  <span>{c.author}</span>
                  <span>{formatRelativeTime(c.date)}</span>
                </div>
              </div>
            ))
          )}
        </div>
      )}

      {/* ── FILE DIFF VIEW ──────────────────────────────── */}
      {currentView.kind === "file-diff" && (
        <div className="git-diff-fullview">
          <div className="git-diff-header">
            <FileText size={12} />
            <span>{currentView.file}</span>
            <span className="git-diff-type">
              {currentView.staged ? "staged" : "unstaged"}
            </span>
          </div>
          <div className="git-diff-body">
            {diffLoading ? (
              <div className="git-loading">
                <Loader2 size={18} className="spin" />
              </div>
            ) : diffOld === "" && diffNew === "" ? (
              <div className="git-empty">
                <span>No diff available</span>
              </div>
            ) : (
              <ReactDiffViewer
                oldValue={diffOld}
                newValue={diffNew}
                splitView={false}
                useDarkTheme={true}
                compareMethod={DiffMethod.LINES}
                styles={diffStyles}
              />
            )}
          </div>
        </div>
      )}

      {/* ── COMMIT DETAIL VIEW (from Log) ───────────────── */}
      {currentView.kind === "commit" && (
        <div className="git-commit-detail">
          {commitDetailLoading ? (
            <div className="git-loading">
              <Loader2 size={18} className="spin" />
            </div>
          ) : commitDetail ? (
            <>
              {/* Commit metadata */}
              <div className="git-commit-meta">
                <div className="git-commit-meta-hash">
                  {commitDetail.hash.slice(0, 10)}
                </div>
                <div className="git-commit-meta-message">
                  {commitDetail.message}
                </div>
                <div className="git-commit-meta-info">
                  <span>{commitDetail.author}</span>
                  <span>{formatRelativeTime(commitDetail.date)}</span>
                </div>
                {commitDetail.files.length > 0 && (
                  <div className="git-commit-files-summary">
                    <span>
                      {commitDetail.files.length} file
                      {commitDetail.files.length !== 1 ? "s" : ""} changed
                    </span>
                    <div className="git-commit-expand-controls">
                      <button
                        className="git-commit-expand-btn"
                        onClick={expandAllFiles}
                        title="Expand all files"
                      >
                        Expand all
                      </button>
                      <button
                        className="git-commit-expand-btn"
                        onClick={collapseAllFiles}
                        title="Collapse all files"
                      >
                        Collapse all
                      </button>
                    </div>
                  </div>
                )}
              </div>

              {/* Per-file diff accordions */}
              <div className="git-commit-file-accordions">
                {(() => {
                  const perFileDiffs = splitDiffByFile(commitDetail.diff || "");
                  return commitDetail.files.map((f) => {
                    const isExpanded = expandedFiles.has(f.path);
                    const fileDiff = perFileDiffs.get(f.path) || "";
                    const fileName = f.path.split("/").pop() || f.path;
                    const dirPath = f.path.includes("/")
                      ? f.path.slice(0, f.path.lastIndexOf("/") + 1)
                      : "";
                    return (
                      <div
                        key={f.path}
                        className={`git-commit-file-accordion ${isExpanded ? "expanded" : ""}`}
                      >
                        <button
                          className="git-commit-file-header"
                          onClick={() => toggleFileAccordion(f.path)}
                        >
                          {isExpanded ? (
                            <ChevronDown size={12} />
                          ) : (
                            <ChevronRight size={12} />
                          )}
                          <span
                            className="git-file-status"
                            style={{ color: statusColor(f.status) }}
                            title={statusLabel(f.status)}
                          >
                            {f.status}
                          </span>
                          <span className="git-commit-file-path">
                            {dirPath && (
                              <span className="git-commit-file-dir">
                                {dirPath}
                              </span>
                            )}
                            <span className="git-commit-file-name">
                              {fileName}
                            </span>
                          </span>
                        </button>
                        {isExpanded && (
                          <div className="git-commit-file-body">
                            {fileDiff ? (
                              <ReactDiffViewer
                                oldValue={
                                  parseUnifiedDiff(fileDiff).oldText
                                }
                                newValue={
                                  parseUnifiedDiff(fileDiff).newText
                                }
                                splitView={false}
                                useDarkTheme={true}
                                compareMethod={DiffMethod.LINES}
                                styles={diffStyles}
                              />
                            ) : (
                              <div className="git-empty git-empty-sm">
                                <span>No diff available for this file</span>
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    );
                  });
                })()}
              </div>
            </>
          ) : (
            <div className="git-empty">
              <span>Failed to load commit</span>
            </div>
          )}
        </div>
      )}

      {/* PR Description modal */}
      {prModalOpen && prModalData && (
        <div className="pr-description-modal-overlay" onClick={() => setPrModalOpen(false)}>
          <div className="pr-description-modal" onClick={(e) => e.stopPropagation()}>
            <div className="pr-description-modal-header">
              <GitPullRequest size={16} />
              <span>PR Context: {prModalData.branch} → {prModalData.base}</span>
              <button className="pr-description-modal-close" onClick={() => setPrModalOpen(false)}>
                <X size={14} />
              </button>
            </div>
            <div className="pr-description-modal-body">
              <div className="pr-description-stats">
                <span>{prModalData.commits.length} commits</span>
                <span>{prModalData.files_changed} files changed</span>
              </div>
              <div className="pr-description-commits">
                <h4>Commits</h4>
                <ul>
                  {prModalData.commits.map((c) => (
                    <li key={c.hash}>
                      <code>{c.hash.slice(0, 7)}</code> {c.message}
                    </li>
                  ))}
                </ul>
              </div>
              <p className="pr-description-hint">
                <Sparkles size={12} /> AI is generating a PR description in the chat panel...
              </p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ── Helpers ──────────────────────────────────────────────

/**
 * Split a unified diff containing multiple files into a Map keyed by file path.
 * Each entry contains the full diff section for that file (including the
 * `diff --git` header, index line, --- / +++ lines, and all hunks).
 */
function splitDiffByFile(fullDiff: string): Map<string, string> {
  const result = new Map<string, string>();
  if (!fullDiff.trim()) return result;

  // Split on `diff --git` boundaries, keeping the delimiter
  const parts = fullDiff.split(/(?=^diff --git )/m);

  for (const part of parts) {
    const trimmed = part.trim();
    if (!trimmed.startsWith("diff --git")) continue;

    // Extract file path from "diff --git a/path b/path"
    const headerMatch = trimmed.match(/^diff --git a\/(.+?) b\/(.+)/m);
    if (headerMatch) {
      // Use the "b/" side as the canonical path (handles renames)
      const filePath = headerMatch[2];
      result.set(filePath, trimmed);
    }
  }

  return result;
}

/**
 * Parse a unified diff into old (removed) and new (added) text for the diff viewer.
 */
function parseUnifiedDiff(diff: string): { oldText: string; newText: string } {
  if (!diff.trim()) return { oldText: "", newText: "" };

  const oldLines: string[] = [];
  const newLines: string[] = [];
  let inHunk = false;

  for (const line of diff.split("\n")) {
    if (line.startsWith("@@")) {
      inHunk = true;
      continue;
    }
    if (!inHunk) continue;

    if (line.startsWith("-")) {
      oldLines.push(line.slice(1));
    } else if (line.startsWith("+")) {
      newLines.push(line.slice(1));
    } else if (line.startsWith(" ")) {
      oldLines.push(line.slice(1));
      newLines.push(line.slice(1));
    } else if (line === "\\ No newline at end of file") {
      // Skip
    } else {
      oldLines.push(line);
      newLines.push(line);
    }
  }

  return {
    oldText: oldLines.join("\n"),
    newText: newLines.join("\n"),
  };
}

/** Format an ISO date string to a relative time like "2 hours ago" */
function formatRelativeTime(isoDate: string): string {
  try {
    const date = new Date(isoDate);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);

    if (diffMins < 1) return "just now";
    if (diffMins < 60) return `${diffMins}m ago`;
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return `${diffHours}h ago`;
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 30) return `${diffDays}d ago`;
    const diffMonths = Math.floor(diffDays / 30);
    if (diffMonths < 12) return `${diffMonths}mo ago`;
    return `${Math.floor(diffMonths / 12)}y ago`;
  } catch {
    return isoDate;
  }
}
