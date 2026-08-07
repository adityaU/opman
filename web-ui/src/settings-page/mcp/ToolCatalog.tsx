import React, { useState } from "react";
import { AlertTriangle, ChevronRight, PlugZap, RotateCw } from "lucide-react";
import type { McpCatalog, McpTool } from "../../api/mcp";
import { signatureOf } from "./schema";
import { ToolDefinition } from "./ToolDefinition";
import { useServerTools } from "./useServerTools";

/**
 * What a server actually offers, under the row that declares it.
 *
 * Two depths, because the two questions are asked at different times. Scanning — "does
 * this server have something for X" — is answered by the list alone: every tool shows its
 * call signature, so the argument names are readable without opening anything. Inspecting
 * one is a click that expands in place rather than a pane that replaces the list, so the
 * user never loses the row they were reading.
 */

export interface ToolCatalogProps {
  readonly name: string;
  /** Mounted only while open — the probe launches a process, so it must not run early. */
  readonly open: boolean;
}

export function ToolCatalog({ name, open }: ToolCatalogProps) {
  const { state, retry } = useServerTools(name, open);
  const [expanded, setExpanded] = useState<string>();

  if (!open || !state) return null;

  if (state.phase === "asking") {
    return (
      <div className="mcpt" aria-busy="true">
        <p className="mcpt-status">
          <PlugZap size={13} aria-hidden="true" />
          Starting <strong>{name}</strong> to ask what it offers…
        </p>
      </div>
    );
  }

  if (state.phase === "unreachable") {
    return <Problem title="opman could not be reached" detail={state.reason} onRetry={retry} />;
  }

  const { catalog } = state;
  if (catalog.status === "unavailable") {
    return <Problem title="Not launchable right now" detail={catalog.reason} onRetry={retry} />;
  }
  if (catalog.status === "failed") {
    return <Problem title={`${name} did not answer`} detail={catalog.reason} onRetry={retry} />;
  }

  return (
    <div className="mcpt">
      <p className="mcpt-status">
        <Count tools={catalog.tools} />
        {catalog.server?.name && catalog.server.name !== name && (
          <span className="mcpt-ident">
            reported as {catalog.server.name}
            {catalog.server.version ? ` ${catalog.server.version}` : ""}
          </span>
        )}
      </p>

      {catalog.tools.length === 0 ? (
        <p className="mcpt-empty">
          It started and answered, but exposes no tools. That is usually a server that
          offers resources or prompts instead.
        </p>
      ) : (
        <ul className="mcpt-list">
          {catalog.tools.map((tool) => (
            <ToolEntry
              key={tool.name}
              tool={tool}
              open={expanded === tool.name}
              onToggle={() =>
                setExpanded((current) => (current === tool.name ? undefined : tool.name))
              }
            />
          ))}
        </ul>
      )}
    </div>
  );
}

function Count({ tools }: { readonly tools: readonly McpTool[] }) {
  return (
    <>
      <strong>{tools.length}</strong> {tools.length === 1 ? "tool" : "tools"}
    </>
  );
}

function ToolEntry({
  tool,
  open,
  onToggle,
}: {
  readonly tool: McpTool;
  readonly open: boolean;
  readonly onToggle: () => void;
}) {
  return (
    <li className={open ? "mcpt-item is-open" : "mcpt-item"}>
      <button type="button" className="mcpt-item-head" onClick={onToggle} aria-expanded={open}>
        <ChevronRight size={13} className="mcpt-chevron" aria-hidden="true" />
        <span className="mcpt-item-text">
          <code className="mcpt-sig">{signatureOf(tool.name, tool.inputSchema)}</code>
          {(tool.title || tool.description) && (
            <span className="mcpt-summary">{tool.title ?? firstSentence(tool.description)}</span>
          )}
        </span>
      </button>
      {open && <ToolDefinition tool={tool} />}
    </li>
  );
}

/**
 * The opening sentence of a description, for the collapsed line.
 *
 * Truncating mid-word would make the list unreadable at exactly the moment it is being
 * skimmed, so this cuts at a sentence and falls back to the whole thing when there is not
 * a clean break to take.
 */
function firstSentence(text: string | undefined): string {
  if (!text) return "";
  const trimmed = text.trim();
  const stop = trimmed.search(/[.!?](\s|$)/);
  return stop > 0 && stop < 160 ? trimmed.slice(0, stop + 1) : trimmed;
}

/**
 * A probe that did not produce a listing.
 *
 * Named separately from "no tools": a server that will not start and a server with nothing
 * to offer look identical in an empty list, and only one of them is a problem to fix.
 */
function Problem({
  title,
  detail,
  onRetry,
}: {
  readonly title: string;
  readonly detail: string;
  readonly onRetry: () => void;
}) {
  return (
    <div className="mcpt">
      <p className="mcpt-problem">
        <AlertTriangle size={13} aria-hidden="true" />
        <span>
          <strong>{title}.</strong> {detail}
        </span>
      </p>
      <button type="button" className="stg-btn mcpt-retry" onClick={onRetry}>
        <RotateCw size={12} aria-hidden="true" />
        Try again
      </button>
    </div>
  );
}
