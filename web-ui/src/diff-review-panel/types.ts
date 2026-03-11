export interface DiffReviewPanelProps {
  onClose: () => void;
  sessionId: string | null;
  /** SSE-driven counter — when it changes, auto-refresh edits */
  fileEditCount: number;
}

export type FileStatus = "accepted" | "reverted" | null;
