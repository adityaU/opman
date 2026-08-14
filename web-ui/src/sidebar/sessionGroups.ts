import type { SessionInfo } from "../api";

/**
 * Recency buckets for the session list.
 *
 * Ordered oldest-last, and used as the render order — a list of forty titles reads
 * as a wall; the same list under "Today" and "Yesterday" reads as a history.
 */
export type SessionBucket = "today" | "yesterday" | "week" | "older";

export const BUCKET_LABEL: Record<SessionBucket, string> = {
  today: "Today",
  yesterday: "Yesterday",
  week: "Previous 7 days",
  older: "Older",
};

const BUCKET_ORDER: SessionBucket[] = ["today", "yesterday", "week", "older"];

const DAY_MS = 86_400_000;

/** Local midnight for the day `at` falls in. */
function startOfDay(at: number): number {
  const date = new Date(at);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

/**
 * Which bucket a timestamp belongs to, relative to `now`.
 *
 * Day boundaries are local midnights rather than rolling 24-hour windows, so a
 * session from 11pm last night reads as "Yesterday" at 1am and not as "Today".
 */
export function bucketFor(updated: number, now: number): SessionBucket {
  const today = startOfDay(now);
  if (updated >= today) return "today";
  if (updated >= today - DAY_MS) return "yesterday";
  if (updated >= today - 7 * DAY_MS) return "week";
  return "older";
}

export interface SessionGroup {
  bucket: SessionBucket;
  label: string;
  sessions: SessionInfo[];
}

/**
 * Split recency-sorted sessions into non-empty buckets, preserving their order.
 *
 * `keepFirst` sessions — pinned ones — are left out of the buckets and handed back
 * separately: a pin means "hold this at the top", which a date heading would undo.
 */
export function groupSessions(
  sessions: SessionInfo[],
  now: number,
  keepFirst?: Set<string>,
): { pinned: SessionInfo[]; groups: SessionGroup[] } {
  const pinned: SessionInfo[] = [];
  const byBucket = new Map<SessionBucket, SessionInfo[]>();

  for (const session of sessions) {
    if (keepFirst?.has(session.id)) {
      pinned.push(session);
      continue;
    }
    const bucket = bucketFor(session.time.updated, now);
    const list = byBucket.get(bucket);
    if (list) {
      list.push(session);
      continue;
    }
    byBucket.set(bucket, [session]);
  }

  const groups: SessionGroup[] = [];
  for (const bucket of BUCKET_ORDER) {
    const found = byBucket.get(bucket);
    if (!found) continue;
    groups.push({ bucket, label: BUCKET_LABEL[bucket], sessions: found });
  }
  return { pinned, groups };
}
