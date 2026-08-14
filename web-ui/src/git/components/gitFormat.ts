/**
 * Small presentation helpers shared by the changes and history views.
 *
 * They live apart from the views because both a file row and a commit's file
 * list need to dim a directory the same way; two copies would drift.
 */

import type { GitAction } from "../types";

/** Split a repo-relative path into its directory prefix and its filename. */
export function splitPath(path: string): { dir: string; name: string } {
  const cut = path.lastIndexOf("/");
  if (cut < 0) return { dir: "", name: path };
  return { dir: path.slice(0, cut + 1), name: path.slice(cut + 1) };
}

/**
 * Wrap an endpoint that answers with no body so it can go through the action
 * runner, which needs a `GitAction` to decide success from refusal.
 */
export function asAction(message: string, operation: () => Promise<void>): () => Promise<GitAction> {
  return async () => {
    await operation();
    return { ok: true, message };
  };
}

const UNITS: Array<[seconds: number, label: string]> = [
  [31536000, "y"],
  [2592000, "mo"],
  [604800, "w"],
  [86400, "d"],
  [3600, "h"],
  [60, "m"],
];

/** "3d ago" from an ISO or git-style date; the raw string if it will not parse. */
export function relativeTime(date: string): string {
  const parsed = Date.parse(date);
  if (Number.isNaN(parsed)) return date;
  const seconds = Math.max(0, (Date.now() - parsed) / 1000);
  for (const [size, label] of UNITS) {
    if (seconds >= size) return `${Math.floor(seconds / size)}${label} ago`;
  }
  return "just now";
}

/** The first line of a commit message, which is all a list row can show. */
export function subjectOf(message: string): string {
  const line = message.split("\n", 1)[0];
  return line.trim() || "(no message)";
}

/** A human name for a porcelain status letter, used as the badge's title. */
export function statusLabel(status: string): string {
  const letter = status.trim().charAt(0).toUpperCase();
  switch (letter) {
    case "A":
      return "Added";
    case "M":
      return "Modified";
    case "D":
      return "Deleted";
    case "R":
      return "Renamed";
    case "C":
      return "Copied";
    case "U":
      return "Unmerged";
    case "?":
      return "Untracked";
    default:
      return status || "Changed";
  }
}
