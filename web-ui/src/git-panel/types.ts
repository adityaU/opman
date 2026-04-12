/**
 * Git panel domain types.
 */
import type { GitFileEntry, GitLogEntry, GitShowResponse, ThemeColors, GitRepoEntry } from "../api";

// ── Navigation ──────────────────────────────────────────

export type GitTab = "changes" | "log";

/**
 * View stack entries for breadcrumb navigation:
 * - "list"       file list (Changes or Log tab)
 * - "file-diff"  viewing diff for a single working-tree file
 * - "commit"     viewing full commit diff from log
 */
export type GitView =
  | { kind: "list" }
  | { kind: "file-diff"; file: string; staged: boolean }
  | { kind: "commit"; hash: string; shortHash: string };

// ── Component props ─────────────────────────────────────

export interface GitPanelProps {
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

// ── PR modal data ───────────────────────────────────────

export interface PRModalData {
  branch: string;
  base: string;
  commits: GitLogEntry[];
  diff: string;
  files_changed: number;
}

// ── Re-exports for convenience ──────────────────────────

export type { GitFileEntry, GitLogEntry, GitShowResponse, ThemeColors, GitRepoEntry };
