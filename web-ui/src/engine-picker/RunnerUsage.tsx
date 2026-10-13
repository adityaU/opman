/**
 * How much a runner has left, drawn inside its row in the engine palette.
 *
 * Meters show what is LEFT, not what is used: the decision being made here is which
 * runner takes the next turn, and "62% left" answers it directly where "38% used" makes
 * the reader do the subtraction. Colour is spent only on trouble — a healthy window is
 * plain ink, so a warning-coloured bar is noticed the moment it appears.
 */
import { AlertTriangle, Clock } from "lucide-react";
import { Skeleton } from "../skeleton/Skeleton";
import type { UsageAccount, UsageWindow } from "../usage/usageClient";
import { formatAge, formatResetIn, noteTone, remainingPercent, severityOf, stateNote } from "../usage/format";

export function UsageMeter({ window, now }: { window: UsageWindow; now: number }) {
  const left = remainingPercent(window);
  const reset = formatResetIn(window.resetsAt, now);
  return (
    <span className="usage-meter" data-severity={severityOf(window)}>
      <span className="usage-meter-label">{window.label}</span>
      <span
        className="usage-meter-track"
        role="meter"
        aria-label={`${window.label} left`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={left}
        aria-valuetext={`${left}% left${reset ? `, ${reset}` : ""}`}
      >
        <span className="usage-meter-fill" style={{ transform: `scaleX(${left / 100})` }} />
      </span>
      <span className="usage-meter-value">{left}%</span>
      <span className="usage-meter-reset">{reset ?? ""}</span>
    </span>
  );
}

/** Two meter-shaped placeholders: the same columns the real meters will fill. */
export function UsageMetersSkeleton() {
  return (
    <span className="usage-meters" data-loading="true">
      {[0, 1].map((row) => (
        <span key={row} className="usage-meter">
          <Skeleton className="usage-meter-label" width={row ? 38 : 44} height={8} />
          <Skeleton className="usage-meter-track" shape="pill" height={4} />
          <Skeleton className="usage-meter-value" width={24} height={8} />
          <Skeleton className="usage-meter-reset" width={row ? 64 : 76} height={8} />
        </span>
      ))}
    </span>
  );
}

export function UsageMeters({ account, now }: { account: UsageAccount; now: number }) {
  if (account.windows.length === 0) return null;
  // An expired login still has its last reading; show it, but as history.
  const dim = account.state !== "ok" || account.stale;
  return (
    <span className={`usage-meters${dim ? " is-dim" : ""}`}>
      {account.windows.slice(0, 3).map((window) => (
        <UsageMeter key={window.id} window={window} now={now} />
      ))}
    </span>
  );
}

/**
 * The meta line under a runner's name: a state note, how old the reading is when the
 * server says it is stale, and a credit balance. Null when there is nothing to add.
 */
export function UsageMeta({ account, now }: { account: UsageAccount | null; now: number }) {
  if (!account) return <span className="engine-runner-meta">Usage not reported</span>;
  const note = stateNote(account);
  const tone = noteTone(account);
  const age = account.stale ? formatAge(account.fetchedAt, now) : null;
  const credits = account.credits?.enabled && account.credits.balance ? account.credits.balance : null;
  if (!note && !account.stale && !credits) return null;
  return (
    <span className="engine-runner-meta">
      {note && (
        <span className={`engine-runner-note is-${tone}`}>
          {tone === "warning" && <AlertTriangle size={11} aria-hidden="true" />}
          <NoteText text={note} />
        </span>
      )}
      {account.stale && (
        <span className="engine-runner-stale" title="The last reading could not be refreshed">
          <Clock size={11} aria-hidden="true" />
          {age ? `Reading from ${age}` : "Out of date"}
        </span>
      )}
      {credits && <span className="engine-runner-credits">Credits {credits}</span>}
    </span>
  );
}

/** Backticked words in a note are commands; set them as code. */
function NoteText({ text }: { text: string }) {
  const parts = text.split(/`([^`]+)`/);
  return (
    <span>
      {parts.map((part, index) => (index % 2 ? <code key={index}>{part}</code> : part))}
    </span>
  );
}
