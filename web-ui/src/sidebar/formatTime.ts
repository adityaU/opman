/**
 * Normalize an epoch timestamp to milliseconds.
 * The server returns Date.now() (ms), but some paths may pass seconds.
 * Heuristic: values above 10 billion are already milliseconds.
 */
export function toMs(ts: number): number {
  return ts > 10_000_000_000 ? ts : ts * 1000;
}

/**
 * Format an epoch timestamp (seconds or milliseconds) as a human-friendly
 * relative time string: "now", "5m ago", "2h ago", "3d ago", or "Mar 21".
 */
export function formatTime(epoch: number): string {
  if (!epoch) return "";
  const d = new Date(toMs(epoch));
  const diffMs = Date.now() - d.getTime();
  if (diffMs < 60_000) return "now";
  const diffMin = Math.floor(diffMs / 60_000);
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHrs = Math.floor(diffMin / 60);
  if (diffHrs < 24) return `${diffHrs}h ago`;
  const diffDays = Math.floor(diffHrs / 24);
  if (diffDays < 7) return `${diffDays}d ago`;
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
