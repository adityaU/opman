import { useState, useCallback, useEffect, useRef } from "react";
import {
  fetchGitStatus,
  fetchGitDiff,
  fetchGitLog,
  fetchGitShow,
  fetchGitBranches,
  fetchTheme,
} from "../../api";
import type { GitFileEntry, GitLogEntry, GitShowResponse, ThemeColors, GitTab, GitView } from "../types";
import { parseUnifiedDiff } from "../utils";

// ── Return type ─────────────────────────────────────────

export interface GitDataState {
  // theme
  themeColors: ThemeColors | null;
  // status
  branch: string;
  setBranch: (b: string) => void;
  staged: GitFileEntry[];
  unstaged: GitFileEntry[];
  untracked: GitFileEntry[];
  loading: boolean;
  refreshStatus: () => Promise<void>;
  // log
  commits: GitLogEntry[];
  logLoading: boolean;
  refreshLog: () => Promise<void>;
  // branches
  localBranches: string[];
  remoteBranches: string[];
  branchesLoading: boolean;
  fetchBranchList: () => Promise<void>;
  // commit detail
  commitDetail: GitShowResponse | null;
  commitDetailLoading: boolean;
  expandedFiles: Set<string>;
  toggleFileAccordion: (path: string) => void;
  expandAllFiles: () => void;
  collapseAllFiles: () => void;
  // file diff
  diffOld: string;
  diffNew: string;
  diffLoading: boolean;
}

// ── Hook ────────────────────────────────────────────────

export function useGitData(
  projectPath: string | null | undefined,
  tab: GitTab,
  currentView: GitView,
  onError?: (msg: string) => void,
): GitDataState {
  // Theme
  const [themeColors, setThemeColors] = useState<ThemeColors | null>(null);

  // Status
  const [branch, setBranch]       = useState("");
  const [staged, setStaged]       = useState<GitFileEntry[]>([]);
  const [unstaged, setUnstaged]   = useState<GitFileEntry[]>([]);
  const [untracked, setUntracked] = useState<GitFileEntry[]>([]);
  const [loading, setLoading]     = useState(false);

  // Log
  const [commits, setCommits]     = useState<GitLogEntry[]>([]);
  const [logLoading, setLogLoading] = useState(false);

  // Branches
  const [localBranches, setLocalBranches]   = useState<string[]>([]);
  const [remoteBranches, setRemoteBranches] = useState<string[]>([]);
  const [branchesLoading, setBranchesLoading] = useState(false);

  // Commit detail
  const [commitDetail, setCommitDetail]           = useState<GitShowResponse | null>(null);
  const [commitDetailLoading, setCommitDetailLoading] = useState(false);
  const [expandedFiles, setExpandedFiles]         = useState<Set<string>>(new Set());

  // File diff
  const [diffOld, setDiffOld]         = useState("");
  const [diffNew, setDiffNew]         = useState("");
  const [diffLoading, setDiffLoading] = useState(false);

  // ── Theme load ────────────────────────────────────────
  useEffect(() => { fetchTheme().then((t) => { if (t) setThemeColors(t); }); }, []);

  // ── Status refresh ────────────────────────────────────
  const refreshStatus = useCallback(async () => {
    setLoading(true);
    try {
      const s = await fetchGitStatus();
      setBranch(s.branch);
      setStaged(s.staged);
      setUnstaged(s.unstaged);
      setUntracked(s.untracked);
    } catch (err) {
      console.error("Failed to fetch git status:", err);
      onError?.("Failed to fetch git status");
    } finally {
      setLoading(false);
    }
  }, [onError]);

  // ── Log refresh ───────────────────────────────────────
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

  // ── Branch list ───────────────────────────────────────
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

  // ── Initial load ──────────────────────────────────────
  useEffect(() => { refreshStatus(); }, [refreshStatus]);

  // ── Project change reset ──────────────────────────────
  const prevProjectPath = useRef(projectPath);
  useEffect(() => {
    if (projectPath === prevProjectPath.current) return;
    prevProjectPath.current = projectPath;
    setBranch(""); setStaged([]); setUnstaged([]); setUntracked([]);
    setCommits([]); setCommitDetail(null);
    setDiffOld(""); setDiffNew("");
    setExpandedFiles(new Set());
    refreshStatus();
    if (tab === "log") refreshLog();
  }, [projectPath, refreshStatus, refreshLog, tab]);

  // ── Auto-load log on tab switch ───────────────────────
  useEffect(() => {
    if (tab === "log" && commits.length === 0) refreshLog();
  }, [tab, commits.length, refreshLog]);

  // ── File diff loading ─────────────────────────────────
  useEffect(() => {
    if (currentView.kind !== "file-diff") return;
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
        setDiffOld(""); setDiffNew("");
      })
      .finally(() => setDiffLoading(false));
  }, [currentView, onError]);

  // ── Commit detail loading ─────────────────────────────
  useEffect(() => {
    if (currentView.kind !== "commit") return;
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
  }, [currentView, onError]);

  // ── Accordion helpers ─────────────────────────────────
  const toggleFileAccordion = useCallback((filePath: string) => {
    setExpandedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(filePath)) next.delete(filePath); else next.add(filePath);
      return next;
    });
  }, []);

  const expandAllFiles = useCallback(() => {
    if (commitDetail?.files) {
      setExpandedFiles(new Set(commitDetail.files.map((f) => f.path)));
    }
  }, [commitDetail]);

  const collapseAllFiles = useCallback(() => { setExpandedFiles(new Set()); }, []);

  return {
    themeColors,
    branch, setBranch,
    staged, unstaged, untracked, loading, refreshStatus,
    commits, logLoading, refreshLog,
    localBranches, remoteBranches, branchesLoading, fetchBranchList,
    commitDetail, commitDetailLoading,
    expandedFiles, toggleFileAccordion, expandAllFiles, collapseAllFiles,
    diffOld, diffNew, diffLoading,
  };
}
