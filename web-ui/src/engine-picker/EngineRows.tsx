/**
 * The palette's row and group vocabulary.
 *
 * Runner rows are the tall ones: a runner is the choice everything else depends on, and
 * it is where the account's remaining usage lives. Model and agent rows stay one line —
 * they are many, and they are read by name.
 */
import { useId, type ReactNode } from "react";
import { Check, RefreshCw } from "lucide-react";
import { agentColor } from "../prompt-input/helpers";
import { SkeletonRows } from "../skeleton/Skeleton";
import { describeUsage, formatAge, planLabel } from "../usage/format";
import type { UsageAccount } from "../usage/usageClient";
import type { UsageView } from "../usage/useUsage";
import type { Row } from "./paletteRows";
import { UsageMeta, UsageMeters, UsageMetersSkeleton } from "./RunnerUsage";

interface RowShell {
  index: number;
  cursor: number;
  onHover: (index: number) => void;
  onChoose: () => void;
}

const rowClass = (selected: boolean, cursor: boolean, extra = "") =>
  `engine-row${extra}${selected ? " is-selected" : ""}${cursor ? " is-cursor" : ""}`;

export function GroupHeader({ icon, title, context, children }: {
  icon: ReactNode;
  title: string;
  context?: string;
  children?: ReactNode;
}) {
  return (
    <div className="engine-group" role="presentation">
      {icon}
      <span className="engine-group-title">{title}</span>
      {context && <span className="engine-group-context">{context}</span>}
      {children && <span className="engine-group-aside">{children}</span>}
    </div>
  );
}

/**
 * The runner header's right side: what the meters measure, how old they are, and a way
 * to re-read. A server without the usage route says so once here instead of on every row.
 */
export function UsageHeaderStatus({ usage, now }: { usage: UsageView; now: number }) {
  if (usage.status === "unavailable") return <span className="engine-usage-status">Usage not available</span>;
  const failed = usage.status === "error" || (usage.error !== null && !usage.refreshing);
  const busy = usage.status === "loading" || usage.refreshing;
  const age = usage.loadedAt ? formatAge(new Date(usage.loadedAt).toISOString(), now) : null;
  return (
    <span className="engine-usage-status">
      {usage.report && <span>% left</span>}
      {failed && <span className="is-warning" title={usage.error ?? undefined}>Couldn’t refresh</span>}
      {!failed && age && !busy && <span>updated {age}</span>}
      <button
        type="button"
        className={`engine-usage-refresh${busy ? " is-busy" : ""}`}
        onClick={() => void usage.refresh()}
        onMouseDown={(event) => event.preventDefault()}
        disabled={busy}
        aria-label={busy ? "Refreshing usage" : "Refresh usage"}
        title="Refresh usage"
      >
        <RefreshCw size={11} aria-hidden="true" />
      </button>
    </span>
  );
}

export function RunnerRow({ row, index, cursor, onHover, onChoose, account, usage, starting, now }: RowShell & {
  row: Extract<Row, { kind: "runner" }>;
  account: UsageAccount | null;
  usage: UsageView;
  starting: boolean;
  now: number;
}) {
  const labelId = useId();
  const detailId = useId();
  const waiting = !usage.report && (usage.status === "loading" || usage.status === "idle");
  const silent = !usage.report && !waiting;
  // The usage as one sentence for assistive tech; the visible parts are a button's
  // presentational children and would be read as a jumble of numbers.
  const description = starting ? "Starting…" : waiting ? "Loading usage" : silent ? "" : describeUsage(account, now);
  return (
    <button
      type="button"
      role="option"
      aria-selected={row.selected}
      aria-busy={starting || undefined}
      // Named by the runner alone: "Claude" must not match "Claude Code", and tests and
      // assistive tech both look runners up by exact name. The usage is the description.
      aria-labelledby={labelId}
      aria-describedby={detailId}
      className={rowClass(row.selected, index === cursor, " engine-runner")}
      onMouseEnter={() => onHover(index)}
      onClick={onChoose}
    >
      <span id={detailId} className="skel-sr-only">{description}</span>
      <span className="engine-runner-id">
        <span className="engine-runner-name">
          <span id={labelId} className="engine-row-label">{row.label}</span>
          {account?.plan && <span className="engine-runner-plan">{planLabel(account.plan)}</span>}
        </span>
        <span className="engine-runner-detail" aria-hidden="true">
          {starting ? (
            <span className="engine-runner-meta is-starting">Starting…</span>
          ) : (
            !waiting && !silent && <UsageMeta account={account} now={now} />
          )}
        </span>
      </span>
      {waiting ? <UsageMetersSkeleton /> : account && <UsageMeters account={account} now={now} />}
      <span className="engine-row-check">{row.selected && <Check size={13} aria-hidden="true" />}</span>
    </button>
  );
}

export function ChoiceRow({ row, index, cursor, onHover, onChoose }: RowShell & {
  row: Exclude<Row, { kind: "runner" }>;
}) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={row.selected}
      className={rowClass(row.selected, index === cursor)}
      onMouseEnter={() => onHover(index)}
      onClick={onChoose}
    >
      {row.kind === "agent" && (
        <span className="engine-row-dot" style={{ backgroundColor: agentColor(row.agent.id, row.agent.color) }} />
      )}
      <span className="engine-row-label">{row.label}</span>
      {row.hint && <span className="engine-row-hint">{row.hint}</span>}
      <span className="engine-row-check">{row.selected && <Check size={13} aria-hidden="true" />}</span>
    </button>
  );
}

/** Placeholder rows with the geometry of the model or agent rows they stand in for. */
export function RowsSkeleton({ kind, label }: { kind: "model" | "agent"; label: string }) {
  return (
    <SkeletonRows
      label={label}
      count={kind === "model" ? 4 : 2}
      rowClassName="engine-row"
      lead={kind === "agent" ? "dot" : "none"}
    />
  );
}
