/**
 * The chip's usage glance: one small gauge for the current runner's tightest window.
 *
 * Only the tightest window, because it is the one that will stop the next turn; the
 * palette has the rest. Nothing at all is drawn when there is nothing to say (no report,
 * a runner that does not report, an API key with no windows) — an empty gauge would read
 * as "0% left". A login that needs attention swaps the gauge for a warning mark.
 */
import { AlertTriangle } from "lucide-react";
import type { UsageAccount } from "../usage/usageClient";
import { describeUsage, mostConstrained, remainingPercent, severityOf } from "../usage/format";

const R = 5;
const C = 2 * Math.PI * R;

export function UsageRing({ account, runnerLabel, now = Date.now() }: {
  account: UsageAccount | null;
  runnerLabel: string;
  now?: number;
}) {
  if (!account) return null;
  const label = `${runnerLabel} usage: ${describeUsage(account, now)}`;

  if (account.state === "expired" || account.state === "signed_out") {
    return (
      <span className="engine-chip-usage is-alert" role="img" aria-label={label} title={label}>
        <AlertTriangle size={11} aria-hidden="true" />
      </span>
    );
  }

  const window = mostConstrained(account);
  if (!window) return null;
  const left = remainingPercent(window);
  return (
    <span
      className={`engine-chip-usage${account.stale ? " is-stale" : ""}`}
      data-severity={severityOf(window)}
      role="img"
      aria-label={label}
      title={label}
    >
      <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
        <circle className="engine-chip-usage-track" cx="7" cy="7" r={R} />
        <circle
          className="engine-chip-usage-arc"
          cx="7"
          cy="7"
          r={R}
          strokeDasharray={`${(C * left) / 100} ${C}`}
          transform="rotate(-90 7 7)"
        />
      </svg>
    </span>
  );
}
