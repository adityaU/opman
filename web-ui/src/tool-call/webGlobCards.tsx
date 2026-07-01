import React, { useMemo, useState, useEffect } from "react";
import { Globe, Search, Folder, ChevronDown, ChevronRight, AlertTriangle, Loader2 } from "lucide-react";
import type { MessagePart } from "../types";
import { str, asObj, asArr, TcStatus } from "./tcUtils";
import { useAutoOpen } from "../hooks/useAutoOpen";

// ── Tool detection ─────────────────────────────────────────────────────────

export function isWebSearchCard(name: string): boolean {
  const n = name.toLowerCase();
  return n.includes("websearch") || n.includes("web_search") || n.includes("searchxng") || n.includes("search_web");
}

export function isWebFetchCard(name: string): boolean {
  const n = name.toLowerCase();
  return n.includes("webfetch") || n.includes("web_fetch") || n.includes("fetch_url") || n.includes("url_fetch");
}

export function isGlobCard(name: string): boolean {
  const n = name.toLowerCase();
  return (
    n.includes("glob") ||
    n.includes("grep") ||
    n.includes("ls") ||
    n.includes("list_dir") ||
    n.includes("find_files") ||
    (n.includes("search") && !n.includes("web") && !n.includes("cross"))
  );
}

// ── WebSearch Card ─────────────────────────────────────────────────────────

interface SearchResult {
  title?: string;
  url?: string;
  snippet?: string;
  description?: string;
  content?: string;
}

function parseSearchResults(output: string): SearchResult[] | null {
  if (!output) return null;
  const t = output.trim();
  try {
    const data = JSON.parse(t);
    if (Array.isArray(data) && data.length > 0 && typeof data[0] === "object") {
      return data as SearchResult[];
    }
    if (data?.results && Array.isArray(data.results)) return data.results as SearchResult[];
  } catch { /* not JSON */ }
  return null;
}

export function WebSearchCard({ part }: { part: MessagePart }) {
  const toolName = part.tool || part.toolName || "unknown";
  const { shouldAutoOpen } = useAutoOpen();
  const [expanded, setExpanded] = useState(() => shouldAutoOpen(toolName));

  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isRunning = status === "running" || status === "pending";
  const input = useMemo(() => asObj(state?.input), [state?.input]);
  const query = str(input.query || input.q || input.search || input.prompt);
  const output = state?.output ?? "";
  const durationMs = state?.time?.start && state?.time?.end ? state.time.end - state.time.start : null;

  const results = useMemo(() => parseSearchResults(output), [output]);
  const toggle = () => setExpanded(e => !e);

  return (
    <div className={`tc-card tc-web${isError ? " tool-call-error" : ""}`}>
      <button className="tc-card-head-btn" onClick={toggle}>
        <span className="tc-card-chevron">{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}</span>
        <Search size={12} className="tc-card-icon" />
        <span className="tc-card-label">WebSearch</span>
        {query && <span className="tc-card-desc">"{query}"</span>}
        <TcStatus status={status} durationMs={durationMs} />
      </button>

      {expanded && (
        <div className="tc-card-body">
          {isError && (
            <div className="tool-call-error-banner">
              <AlertTriangle size={12} />
              <span>{state?.error || "Search failed"}</span>
            </div>
          )}

          {!isError && results && results.length > 0 && (
            <div className="tc-search-results">
              {results.slice(0, 8).map((r, i) => (
                <div key={i} className="tc-search-result">
                  {r.title && <div className="tc-search-result-title">{r.title}</div>}
                  {r.url && <div className="tc-search-result-url">{r.url}</div>}
                  {(r.snippet || r.description || r.content) && (
                    <div className="tc-search-result-snippet">
                      {r.snippet || r.description || r.content}
                    </div>
                  )}
                </div>
              ))}
              {results.length > 8 && (
                <div className="tc-search-count">+ {results.length - 8} more results</div>
              )}
            </div>
          )}

          {!isError && output && !results && (
            <pre className="gmc-pre">{output.slice(0, 2000)}{output.length > 2000 ? "\n…" : ""}</pre>
          )}

          {!isError && isRunning && !output && (
            <div className="tc-card-muted">
              <Loader2 size={11} className="tool-spin-icon" /> Searching…
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ── WebFetch Card ──────────────────────────────────────────────────────────

export function WebFetchCard({ part }: { part: MessagePart }) {
  const toolName = part.tool || part.toolName || "unknown";
  const { shouldAutoOpen } = useAutoOpen();
  const [expanded, setExpanded] = useState(() => shouldAutoOpen(toolName));

  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isRunning = status === "running" || status === "pending";
  const input = useMemo(() => asObj(state?.input), [state?.input]);
  const url = str(input.url || input.href || input.uri);
  const output = state?.output ?? "";
  const durationMs = state?.time?.start && state?.time?.end ? state.time.end - state.time.start : null;

  // Extract a clean domain for display
  const domain = useMemo(() => {
    try { return new URL(url).hostname; } catch { return url; }
  }, [url]);

  const toggle = () => setExpanded(e => !e);

  return (
    <div className={`tc-card tc-web${isError ? " tool-call-error" : ""}`}>
      <button className="tc-card-head-btn" onClick={toggle}>
        <span className="tc-card-chevron">{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}</span>
        <Globe size={12} className="tc-card-icon" />
        <span className="tc-card-label">WebFetch</span>
        {domain && <span className="tc-card-path">{domain}</span>}
        <TcStatus status={status} durationMs={durationMs} />
      </button>

      {expanded && (
        <div className="tc-card-body">
          {url && (
            <div className="tc-bash-cmd" style={{ borderColor: "var(--tc-accent, var(--color-border-subtle))" }}>
              <Globe size={10} style={{ color: "var(--tc-accent)", flexShrink: 0 }} />
              <code style={{ wordBreak: "break-all" }}>{url}</code>
            </div>
          )}

          {isError && (
            <div className="tool-call-error-banner">
              <AlertTriangle size={12} />
              <span>{state?.error || "Fetch failed"}</span>
            </div>
          )}

          {!isError && output && (
            <pre className="gmc-pre">{output.slice(0, 3000)}{output.length > 3000 ? "\n…" : ""}</pre>
          )}

          {!isError && isRunning && !output && (
            <div className="tc-card-muted">
              <Loader2 size={11} className="tool-spin-icon" /> Fetching…
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ── Glob / Grep / LS Card ──────────────────────────────────────────────────

function parseFileList(output: string): string[] | null {
  if (!output) return null;
  const t = output.trim();
  try {
    const data = JSON.parse(t);
    if (Array.isArray(data) && data.every(i => typeof i === "string")) return data as string[];
  } catch { /* not JSON */ }
  // Newline-separated paths
  const lines = t.split("\n").map(l => l.trim()).filter(l => l.length > 0 && !l.startsWith("{"));
  if (lines.length > 0) return lines;
  return null;
}

export function GlobCard({ part }: { part: MessagePart }) {
  const toolName = part.tool || part.toolName || "unknown";
  const { shouldAutoOpen } = useAutoOpen();
  const [expanded, setExpanded] = useState(() => shouldAutoOpen(toolName));

  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isRunning = status === "running" || status === "pending";
  const input = useMemo(() => asObj(state?.input), [state?.input]);
  const pattern = str(input.pattern || input.glob || input.regex || input.query);
  const path = str(input.path || input.dir || input.directory || input.cwd);
  const output = state?.output ?? "";
  const durationMs = state?.time?.start && state?.time?.end ? state.time.end - state.time.start : null;

  const files = useMemo(() => parseFileList(output), [output]);

  const lname = toolName.toLowerCase();
  const label = lname.includes("grep") ? "Grep" : lname.includes("ls") || lname.includes("list") ? "LS" : "Glob";

  const toggle = () => setExpanded(e => !e);

  return (
    <div className={`tc-card tc-glob${isError ? " tool-call-error" : ""}`}>
      <button className="tc-card-head-btn" onClick={toggle}>
        <span className="tc-card-chevron">{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}</span>
        <Folder size={12} className="tc-card-icon" />
        <span className="tc-card-label">{label}</span>
        {pattern && <span className="tc-card-desc">{pattern}</span>}
        {files && <span className="tc-card-badge">{files.length} {files.length === 1 ? "match" : "matches"}</span>}
        <TcStatus status={status} durationMs={durationMs} />
      </button>

      {expanded && (
        <div className="tc-card-body">
          {(pattern || path) && (
            <div className="tc-bash-cmd">
              <Folder size={10} style={{ color: "var(--tc-accent)", flexShrink: 0 }} />
              <code>{[pattern, path].filter(Boolean).join("  in  ")}</code>
            </div>
          )}

          {isError && (
            <div className="tool-call-error-banner">
              <AlertTriangle size={12} />
              <span>{state?.error || "Search failed"}</span>
            </div>
          )}

          {!isError && files && files.length > 0 && (
            <div className="tc-glob-list">
              {files.slice(0, 100).map((f, i) => (
                <div key={i} className="tc-glob-item">{f}</div>
              ))}
              {files.length > 100 && (
                <div className="tc-glob-count">… and {files.length - 100} more</div>
              )}
            </div>
          )}

          {!isError && output && !files && (
            <pre className="gmc-pre">{output.slice(0, 2000)}</pre>
          )}

          {!isError && isRunning && !output && (
            <div className="tc-card-muted">
              <Loader2 size={11} className="tool-spin-icon" /> Searching…
            </div>
          )}
        </div>
      )}
    </div>
  );
}
