/**
 * One process-wide copy of the usage report.
 *
 * Every composer on screen carries an engine chip, and a workspace can hold four of them;
 * each chip and each palette reads usage. A module-level store means one request serves
 * all of them, and reopening the palette within a minute paints the meters immediately
 * instead of flashing a skeleton for numbers it already had.
 *
 * Freshness is a client clock (`loadedAt`), separate from the server's own `fetchedAt`:
 * the server may legitimately hand back a cached reading, which is what `stale` is for.
 */
import { fetchUsage, type UsageReport } from "./usageClient";

export const USAGE_FRESH_MS = 60_000;

export type UsageStatus = "idle" | "loading" | "ready" | "unavailable" | "error";

export interface UsageSnapshot {
  /** The last good report. Kept through a failed refresh — old numbers beat none. */
  report: UsageReport | null;
  /** `ready` whenever a report is held; otherwise what the last attempt concluded. */
  status: UsageStatus;
  /** The last attempt's failure, even when an older report is still shown. */
  error: string | null;
  /** Client time of the last completed attempt that settled the status. */
  loadedAt: number | null;
  /** A request is in flight while a report is already on screen. */
  refreshing: boolean;
}

const INITIAL: UsageSnapshot = { report: null, status: "idle", error: null, loadedAt: null, refreshing: false };

let snapshot: UsageSnapshot = INITIAL;
const listeners = new Set<() => void>();
let inflight: Promise<void> | null = null;
let inflightForced = false;
/** Bumped per request so an older answer can never overwrite a newer one. */
let generation = 0;

function publish(next: UsageSnapshot) {
  snapshot = next;
  for (const listener of listeners) listener();
}

export function getUsageSnapshot(): UsageSnapshot {
  return snapshot;
}

export function subscribeUsage(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** True when the held answer is recent enough that asking again would be waste. */
export function isUsageFresh(now = Date.now()): boolean {
  const settled = snapshot.status === "ready" || snapshot.status === "unavailable";
  return settled && snapshot.loadedAt !== null && now - snapshot.loadedAt < USAGE_FRESH_MS;
}

/**
 * Load the report unless a fresh one is held. `force` bypasses both the client freshness
 * window and the server cache (`?refresh=1`) — the palette's refresh button.
 */
export function loadUsage(options: { force?: boolean } = {}): Promise<void> {
  const force = options.force === true;
  if (!force && isUsageFresh()) return Promise.resolve();
  // Join a request already under way, unless this one asks for more than it will deliver.
  if (inflight && (!force || inflightForced)) return inflight;

  const mine = ++generation;
  inflightForced = force;
  publish({
    ...snapshot,
    status: snapshot.report ? "ready" : "loading",
    refreshing: snapshot.report !== null,
  });

  const request = fetchUsage({ refresh: force }).then((result) => {
    if (mine !== generation) return;
    const loadedAt = Date.now();
    if (result.kind === "ok") {
      publish({ report: result.report, status: "ready", error: null, loadedAt, refreshing: false });
    } else if (result.kind === "unavailable") {
      publish({ report: null, status: "unavailable", error: null, loadedAt, refreshing: false });
    } else {
      publish({
        report: snapshot.report,
        status: snapshot.report ? "ready" : "error",
        error: result.message,
        // An error does not reset the freshness clock: the next open should retry.
        loadedAt: snapshot.loadedAt,
        refreshing: false,
      });
    }
  }).finally(() => {
    if (mine === generation) {
      inflight = null;
      inflightForced = false;
    }
  });
  inflight = request;
  return request;
}

/** Tests only: forget everything, including requests still in flight. */
export function resetUsageStore() {
  generation += 1;
  inflight = null;
  inflightForced = false;
  snapshot = INITIAL;
  listeners.clear();
}
