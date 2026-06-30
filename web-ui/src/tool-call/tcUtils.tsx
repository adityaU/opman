import React from "react";
import { CheckCircle2, XCircle, Loader2, Clock } from "lucide-react";
import { formatDuration } from "./helpers";

// ── Shared utilities for tool-specific card renderers ─────────────────────

export const str = (v: unknown): string => (typeof v === "string" ? v : "");

export const asObj = (v: unknown): Record<string, unknown> => {
  if (v && typeof v === "object" && !Array.isArray(v)) return v as Record<string, unknown>;
  if (typeof v === "string") {
    try {
      const p = JSON.parse(v);
      if (p && typeof p === "object" && !Array.isArray(p)) return p as Record<string, unknown>;
    } catch { /* not JSON */ }
  }
  return {};
};

export const asArr = (v: unknown): unknown[] => {
  if (Array.isArray(v)) return v;
  if (typeof v === "string") {
    try {
      const p = JSON.parse(v);
      if (Array.isArray(p)) return p;
    } catch { /* not JSON */ }
  }
  return [];
};

interface TcStatusProps {
  status: string;
  durationMs: number | null;
}

export function TcStatus({ status, durationMs }: TcStatusProps) {
  return (
    <span className="tc-card-status">
      {durationMs != null && (
        <span className="tc-card-duration">
          <Clock size={10} />
          {formatDuration(durationMs)}
        </span>
      )}
      {status === "completed" ? (
        <CheckCircle2 size={12} className="tool-success-icon" />
      ) : status === "error" ? (
        <XCircle size={12} className="tool-error-icon" />
      ) : (
        <Loader2 size={12} className="tool-spin-icon" />
      )}
    </span>
  );
}
