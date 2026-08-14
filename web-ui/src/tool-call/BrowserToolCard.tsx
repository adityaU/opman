import React, { useMemo, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Camera,
  Globe,
  Keyboard,
  Loader2,
  MousePointerClick,
  MoveVertical,
  PanelsTopLeft,
  RotateCw,
  ScanText,
  TextCursorInput,
} from "lucide-react";
import type { MessagePart } from "../types";
import { OutlineView, parseOutline, actionableCount } from "./browserOutline";
import {
  browserAction,
  hostOf,
  parsePageResult,
  parsePanes,
  parseScreenshot,
  pathOf,
  type BrowserAction,
} from "./browserParse";
import { asObj, str } from "./KanbanToolCard";

export { isBrowserTool } from "./browserParse";

/**
 * The `browser_*` MCP tools, as cards.
 *
 * A browser call has one identity a reader cares about — *which page* — and one
 * payload — what the agent saw or did there. The generic card buries the first
 * in a JSON argument blob and dumps the second as preformatted text. Here the
 * host leads, the action names itself, and the outline renders as the structure
 * it actually is.
 *
 * Collapsed by default: in a transcript the fact of the call is the message, and
 * a page outline that expands itself is a wall between the reader and the next
 * message. Expanding is one click, and screenshots preview at a glance because
 * an image is faster to judge than to open.
 */

const ACTION_META: Record<
  BrowserAction,
  { label: string; Icon: typeof Globe; tone: string }
> = {
  open: { label: "Browser", Icon: Globe, tone: "go" },
  navigate: { label: "Browser · History", Icon: RotateCw, tone: "go" },
  snapshot: { label: "Browser · Read", Icon: ScanText, tone: "read" },
  read: { label: "Browser · Text", Icon: ScanText, tone: "read" },
  click: { label: "Browser · Click", Icon: MousePointerClick, tone: "act" },
  type: { label: "Browser · Type", Icon: TextCursorInput, tone: "act" },
  key: { label: "Browser · Key", Icon: Keyboard, tone: "act" },
  scroll: { label: "Browser · Scroll", Icon: MoveVertical, tone: "read" },
  screenshot: { label: "Browser · Shot", Icon: Camera, tone: "read" },
  panes: { label: "Browser · Sessions", Icon: PanelsTopLeft, tone: "read" },
  other: { label: "Browser", Icon: Globe, tone: "go" },
};

export function BrowserToolCard({ part }: { part: MessagePart }) {
  const toolName = part.tool || part.toolName || "";
  const action = browserAction(toolName);
  const meta = ACTION_META[action];

  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isRunning = status === "running" || status === "pending";

  const input = useMemo(() => asObj(state?.input), [state?.input]);
  const output = useMemo(() => {
    const value = state?.output;
    return typeof value === "string" && value.length > 0 ? value : null;
  }, [state?.output]);

  const page = useMemo(
    () => (action === "screenshot" || action === "panes" ? null : parsePageResult(output)),
    [action, output],
  );
  const shot = useMemo(() => (action === "screenshot" ? parseScreenshot(output) : null), [action, output]);
  const panes = useMemo(() => (action === "panes" ? parsePanes(output) : []), [action, output]);

  // The URL the agent asked for is known before the page answers, so a running
  // call still names its destination instead of showing an empty header.
  const url = page?.url || shot?.url || str(input.url);
  const host = hostOf(url);

  const outlineRows = useMemo(
    () => (page && isOutline(action) ? parseOutline(page.body) : []),
    [action, page],
  );
  const refs = actionableCount(outlineRows);

  const [expanded, setExpanded] = useState(false);
  const canExpand = Boolean(page?.body || panes.length > 0 || shot);

  return (
    <div className={`bwc-card bwc-${meta.tone}${isError ? " bwc-card-error" : ""}`}>
      <button
        type="button"
        className="bwc-head"
        onClick={() => canExpand && setExpanded((open) => !open)}
        aria-expanded={canExpand ? expanded : undefined}
        disabled={!canExpand}
      >
        {canExpand && (
          <span className="bwc-chevron">
            {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
          </span>
        )}
        <span className="bwc-icon">
          <meta.Icon size={13} />
        </span>
        <span className="bwc-label">{meta.label}</span>

        {host && <span className="bwc-host">{host}</span>}
        <Target action={action} input={input} />

        <span className="bwc-status">
          {isError ? (
            <AlertTriangle size={12} className="tool-error-icon" />
          ) : isRunning ? (
            <Loader2 size={12} className="tool-spin-icon" />
          ) : (
            <CheckCircle2 size={12} className="tool-success-icon" />
          )}
        </span>
      </button>

      {expanded && canExpand && (
        <div className="bwc-body">
          {page?.title && <div className="bwc-title">{page.title}</div>}
          {url && (
            <a className="bwc-url" href={url} target="_blank" rel="noreferrer noopener">
              {host}
              <span className="bwc-path">{pathOf(url)}</span>
            </a>
          )}

          {shot && (
            <img
              className="bwc-shot"
              src={`data:image/jpeg;base64,${shot.data}`}
              alt={`Screenshot of ${host}`}
            />
          )}

          {panes.length > 0 && (
            <div className="bwc-panes">
              {panes.map((pane) => (
                <div key={pane.project + pane.url} className="bwc-pane">
                  <span className="bwc-pane-project">{basename(pane.project)}</span>
                  <span className="bwc-pane-title">{pane.title}</span>
                  <span className="bwc-pane-url">{hostOf(pane.url)}</span>
                </div>
              ))}
            </div>
          )}

          {outlineRows.length > 0 && <OutlineView outline={page?.body ?? ""} />}

          {page && !isOutline(action) && page.body && (
            <div className="bwc-prose">{page.body}</div>
          )}

          {page?.truncated && (
            <div className="bwc-truncated">Truncated — the agent saw this much.</div>
          )}

          {isError && (
            <div className="bwc-errmsg">
              <AlertTriangle size={12} />
              <span>{state?.error || "The browser call failed"}</span>
            </div>
          )}
        </div>
      )}

      {!expanded && refs > 0 && (
        <div className="bwc-foot">
          {refs} element{refs === 1 ? "" : "s"} the agent could act on
        </div>
      )}
    </div>
  );
}

/**
 * What the action was aimed at — the ref it clicked, the text it typed, the key
 * it pressed. Shown in the header because it is the difference between two
 * otherwise identical rows in a transcript.
 */
const Target: React.FC<{
  readonly action: BrowserAction;
  readonly input: Record<string, unknown>;
}> = ({ action, input }) => {
  if (action === "click") {
    const ref = str(input.ref);
    return ref ? <span className="bwc-chip is-ref">{ref}</span> : null;
  }
  if (action === "type") {
    const text = str(input.text);
    return (
      <>
        {str(input.ref) && <span className="bwc-chip is-ref">{str(input.ref)}</span>}
        {text && <span className="bwc-typed">“{clip(text, 40)}”</span>}
      </>
    );
  }
  if (action === "key") {
    const key = str(input.key);
    return key ? <span className="bwc-chip is-key">{key}</span> : null;
  }
  if (action === "navigate") {
    const direction = str(input.direction);
    return direction ? <span className="bwc-chip">{direction}</span> : null;
  }
  return null;
};

/** Actions whose body is a `[ref=eN]` outline rather than prose. */
function isOutline(action: BrowserAction): boolean {
  return action === "open" || action === "snapshot" || action === "click"
    || action === "type" || action === "key" || action === "navigate";
}

function clip(value: string, max: number): string {
  return value.length > max ? `${value.slice(0, max)}…` : value;
}

function basename(path: string): string {
  const parts = path.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}
