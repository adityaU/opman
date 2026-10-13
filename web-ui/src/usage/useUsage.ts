import { useCallback, useEffect, useSyncExternalStore } from "react";
import { getUsageSnapshot, loadUsage, subscribeUsage, type UsageSnapshot } from "./usageStore";
import { accountForRunner } from "./format";
import type { UsageAccount } from "./usageClient";

const REFRESH_EVERY_MS = 60_000;

export interface UsageView extends UsageSnapshot {
  /** Re-read now, past both the client and the server cache. */
  refresh: () => Promise<void>;
}

/**
 * The shared usage report. Mounting asks for it once (a no-op while it is fresh); pass
 * `load: false` to only observe what others fetched.
 */
export function useUsage(options: { load?: boolean } = {}): UsageView {
  const load = options.load !== false;
  const snapshot = useSyncExternalStore(subscribeUsage, getUsageSnapshot, getUsageSnapshot);
  useEffect(() => {
    if (!load) return;
    void loadUsage();
    // Keep a long-open composer's reading current. `loadUsage` is a no-op while the shared
    // report is fresh, so many mounted chips still make one request a minute.
    const tick = () => {
      if (document.visibilityState === "visible") void loadUsage();
    };
    const timer = window.setInterval(tick, REFRESH_EVERY_MS);
    document.addEventListener("visibilitychange", tick);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", tick);
    };
  }, [load]);
  const refresh = useCallback(() => loadUsage({ force: true }), []);
  return { ...snapshot, refresh };
}

/** The account a runner bills against, or null when that runner reports no usage. */
export function useRunnerUsage(runner: string, options: { load?: boolean } = {}): {
  account: UsageAccount | null;
  usage: UsageView;
} {
  const usage = useUsage(options);
  return { account: accountForRunner(usage.report, runner), usage };
}
