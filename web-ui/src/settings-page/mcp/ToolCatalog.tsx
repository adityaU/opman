import React, { useState } from "react";
import { AlertTriangle, ChevronRight, PlugZap, RotateCw, Search, X } from "lucide-react";
import type { McpTool } from "../../api/mcp";
import { signatureOf } from "./schema";
import { ToolDefinition } from "./ToolDefinition";
import { useServerTools } from "./useServerTools";
import { CodeModeSwitch } from "./CodeModeSwitch";

interface ToolControls {
  readonly disabledTools: readonly string[];
  readonly busy: boolean;
  readonly onToggle: (tool: string, enabled: boolean) => void;
}

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
  readonly controls?: ToolControls;
}

export function toolCatalogId(name: string): string {
  return `mcpt-list-${encodeURIComponent(name)}`;
}

export function ToolCatalog({ name, open, controls }: ToolCatalogProps) {
  const { state, retry } = useServerTools(name, open);
  const [expanded, setExpanded] = useState<string>();
  const [query, setQuery] = useState("");

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

  const normalizedQuery = query.trim().toLowerCase();
  const tools = normalizedQuery
    ? catalog.tools.filter((tool) =>
        [tool.name, tool.title, tool.description, signatureOf(tool.name, tool.inputSchema)]
          .filter((value): value is string => typeof value === "string")
          .some((value) => value.toLowerCase().includes(normalizedQuery)),
      )
    : catalog.tools;
  const catalogId = `${toolCatalogId(name)}${controls ? "-code-mode" : ""}`;

  return (
    <section className="mcpt" aria-labelledby={`${catalogId}-heading`}>
      <h3 className="mcpt-heading" id={`${catalogId}-heading`}>
        {name === "opman-management" ? "Opman management tools" : `${name} tools`}
      </h3>
      <p className="mcpt-status">
        <Count shown={tools.length} total={catalog.tools.length} />
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
        <>
          {catalog.tools.length > 8 && (
            <div className="mcpt-filter">
              <label className="mcpt-filter-label" htmlFor={`${catalogId}-search`}>
                <Search size={14} aria-hidden="true" />
                <span>Filter tools</span>
              </label>
              <div className="mcpt-filter-control">
                <input
                  id={`${catalogId}-search`}
                  type="search"
                  value={query}
                  onChange={(event) => setQuery(event.currentTarget.value)}
                  placeholder="Name or description"
                  aria-controls={catalogId}
                />
                {query.trim() && (
                  <button
                    type="button"
                    className="mcpt-filter-clear"
                    onClick={() => setQuery("")}
                    aria-label="Clear tool filter"
                  >
                    <X size={14} aria-hidden="true" />
                  </button>
                )}
              </div>
              {normalizedQuery && (
                <span className="mcpt-filter-count" role="status" aria-live="polite">
                  {tools.length} of {catalog.tools.length} tools
                </span>
              )}
            </div>
          )}
          <div className="mcpt-results" id={catalogId}>
            {tools.length === 0 ? (
              <p className="mcpt-empty">No tools match “{query.trim()}”.</p>
            ) : (
              <ul className="mcpt-list" aria-label={`${name} tools`}>
                {tools.map((tool) => (
                  <ToolEntry
                    key={tool.name}
                    tool={tool}
                    controls={controls}
                    serverName={name}
                    definitionId={`${catalogId}-${encodeURIComponent(tool.name)}`}
                    open={expanded === tool.name}
                    onToggle={() =>
                      setExpanded((current) => (current === tool.name ? undefined : tool.name))
                    }
                  />
                ))}
              </ul>
            )}
          </div>
        </>
      )}
    </section>
  );
}

function Count({ shown, total }: { readonly shown: number; readonly total: number }) {
  return (
    <>
      <strong>{shown}</strong> of {total} {total === 1 ? "tool" : "tools"}
    </>
  );
}

function ToolEntry({
  tool,
  definitionId,
  open,
  onToggle,
  controls,
  serverName,
}: {
  readonly tool: McpTool;
  readonly definitionId: string;
  readonly open: boolean;
  readonly onToggle: () => void;
  readonly controls?: ToolControls;
  readonly serverName: string;
}) {
  return (
    <li className={open ? "mcpt-item is-open" : "mcpt-item"}>
      <div className="cm-settings-tool-head">
      <button
        type="button"
        className="mcpt-item-head"
        onClick={onToggle}
        aria-expanded={open}
        aria-controls={definitionId}
      >
        <ChevronRight size={13} className="mcpt-chevron" aria-hidden="true" />
        <span className="mcpt-item-text">
          <code className="mcpt-sig">{signatureOf(tool.name, tool.inputSchema)}</code>
          {(tool.title || tool.description) && (
            <span className="mcpt-summary">{tool.title ?? firstSentence(tool.description)}</span>
          )}
        </span>
      </button>
      {controls && <CodeModeSwitch checked={!controls.disabledTools.includes(tool.name)}
        label={`Use ${serverName}.${tool.name} in code mode`} busy={controls.busy}
        onChange={enabled => controls.onToggle(tool.name, enabled)} />}
      </div>
      {open && <ToolDefinition tool={tool} id={definitionId} />}
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
