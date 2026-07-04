import React, { useMemo, useState, useEffect } from "react";
import { FileText, Terminal, Pencil, AlertTriangle, Loader2, ChevronDown, ChevronRight } from "lucide-react";
import { PrismLight as SyntaxHighlighter } from "react-syntax-highlighter";
import type { MessagePart } from "../types";
import { parseOutput, guessLanguage } from "./helpers";
import { str, asObj, TcStatus } from "./tcUtils";
import { EditDiffView } from "./components";
import { useAutoOpen, classifyTool } from "../hooks/useAutoOpen";
import { useOpenFileInEditor } from "./EditorOpenContext";

// Clickable file path that opens the file in the editor panel. Renders as a plain
// span (not a button) since it lives inside the card's toggle <button>; stops
// propagation so opening the file doesn't also toggle the accordion.
function FilePath({ path, line }: { path: string; line?: number | null }) {
  const openFile = useOpenFileInEditor();
  if (!openFile) return <span className="tc-card-path">{path}</span>;
  const open = (e: React.MouseEvent | React.KeyboardEvent) => {
    e.stopPropagation();
    e.preventDefault();
    openFile(path, line);
  };
  return (
    <span
      className="tc-card-path tc-card-path-link"
      role="link"
      tabIndex={0}
      title={`Open ${path} in editor`}
      onClick={open}
      onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") open(e); }}
    >
      {path}
    </span>
  );
}

// ── Read Tool Card ─────────────────────────────────────────────────────────

export function isReadTool(name: string): boolean {
  const n = name.toLowerCase();
  return n.includes("read") && !n.includes("neovim") && !n.includes("todo") && !n.includes("note");
}

const CODE_STYLE = {
  margin: 0,
  maxHeight: 300,
  overflow: "auto" as const,
  fontSize: "0.7rem",
  fontFamily: "var(--font-mono)",
};
const CODE_TAG = { style: { fontFamily: "var(--font-mono)" } };

export function ReadCard({ part }: { part: MessagePart }) {
  const toolName = part.tool || part.toolName || "unknown";
  const { shouldAutoOpen } = useAutoOpen();
  const [expanded, setExpanded] = useState(() => shouldAutoOpen(toolName));
  const [userToggled, setUserToggled] = useState(false);

  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isRunning = status === "running" || status === "pending";
  const input = useMemo(() => asObj(state?.input), [state?.input]);
  const filePath = str(input.file_path || input.filePath || input.path);
  const offset = input.offset != null ? Number(input.offset) : null;
  const limit = input.limit != null ? Number(input.limit) : null;
  const output = state?.output ?? "";
  const durationMs = state?.time?.start && state?.time?.end ? state.time.end - state.time.start : null;
  const parsed = useMemo(() => parseOutput(output), [output]);
  const lang = guessLanguage(filePath || parsed.path);
  const displayPath = filePath || parsed.path || "file";

  const toggle = () => { setUserToggled(true); setExpanded(e => !e); };

  return (
    <div className={`tc-card tc-read${isError ? " tool-call-error" : ""}`}>
      <button className="tc-card-head tc-card-head-btn" onClick={toggle}>
        <span className="tc-card-chevron">{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}</span>
        <FileText size={12} className="tc-card-icon" />
        <span className="tc-card-label">Read</span>
        {filePath || parsed.path ? (
          <FilePath path={(filePath || parsed.path)!} line={offset != null ? offset + 1 : null} />
        ) : (
          <span className="tc-card-path">{displayPath}</span>
        )}
        {offset != null && (
          <span className="tc-card-badge">
            L{offset}–{limit != null ? offset + limit : "end"}
          </span>
        )}
        <TcStatus status={status} durationMs={durationMs} />
      </button>

      {expanded && isError && (
        <div className="tc-card-body">
          <div className="tool-call-error-banner">
            <AlertTriangle size={12} />
            <span>{state?.error || "Read failed"}</span>
          </div>
        </div>
      )}

      {expanded && !isError && output.length > 0 && (
        <div className="tc-card-body tc-card-body-flush">
          {parsed.type === "file" ? (
            <SyntaxHighlighter
              useInlineStyles={false}
              language={lang}
              PreTag="div"
              showLineNumbers
              codeTagProps={CODE_TAG}
              customStyle={CODE_STYLE}
            >
              {parsed.content}
            </SyntaxHighlighter>
          ) : (
            <pre className="tool-call-pre" style={{ maxHeight: 280, borderRadius: 0, border: "none" }}>
              {output}
            </pre>
          )}
        </div>
      )}

      {expanded && !isError && isRunning && !output && (
        <div className="tc-card-body">
          <div className="tc-card-muted">
            <Loader2 size={11} className="tool-spin-icon" /> Reading…
          </div>
        </div>
      )}
    </div>
  );
}

// ── Bash Tool Card (non-background) ────────────────────────────────────────

export function isBashCard(name: string): boolean {
  const n = name.toLowerCase();
  return (n.includes("bash") || n.includes("shell")) && !n.includes("neovim");
}

export function BashCard({ part }: { part: MessagePart }) {
  const toolName = part.tool || part.toolName || "unknown";
  const { shouldAutoOpen } = useAutoOpen();
  const [expanded, setExpanded] = useState(() => shouldAutoOpen(toolName));
  const [userToggled, setUserToggled] = useState(false);

  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isRunning = status === "running" || status === "pending";
  const input = useMemo(() => asObj(state?.input), [state?.input]);
  const command =
    str(input.command || input.cmd) ||
    (typeof state?.input === "string" ? state.input : "");
  const description = str(input.description);
  const finalOutput = state?.output ?? "";
  const liveOutput =
    typeof state?.metadata?.output === "string" ? state.metadata.output : "";
  const output = finalOutput.length > 0 ? finalOutput : liveOutput;
  const durationMs = state?.time?.start && state?.time?.end ? state.time.end - state.time.start : null;
  const liveRef = React.useRef<HTMLPreElement>(null);

  // Auto-expand when running (matches old accordion behavior)
  useEffect(() => {
    if (!userToggled && isRunning) setExpanded(true);
  }, [userToggled, isRunning]);

  useEffect(() => {
    if (isRunning && liveRef.current) liveRef.current.scrollTop = liveRef.current.scrollHeight;
  }, [output, isRunning]);

  const toggle = () => { setUserToggled(true); setExpanded(e => !e); };

  return (
    <div className={`tc-card tc-bash${isError ? " tool-call-error" : ""}`}>
      <button className="tc-card-head tc-card-head-btn" onClick={toggle}>
        <span className="tc-card-chevron">{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}</span>
        <Terminal size={12} className="tc-card-icon" />
        <span className="tc-card-label">Bash</span>
        {description && <span className="tc-card-desc">{description}</span>}
        {!expanded && command && <span className="tc-card-cmd-preview">$ {command.slice(0, 60)}{command.length > 60 ? "…" : ""}</span>}
        <TcStatus status={status} durationMs={durationMs} />
      </button>

      {expanded && (
        <div className="tc-card-body">
          {command && (
            <div className="tc-bash-cmd">
              <span className="tc-bash-prompt">$</span>
              <code>{command}</code>
            </div>
          )}

          {output.length > 0 && (
            <pre
              ref={liveRef}
              className={`tool-call-pre${isRunning ? " tool-call-live-output" : ""}`}
              style={{ maxHeight: 300 }}
            >
              {output}
            </pre>
          )}

          {!output && isRunning && (
            <div className="tc-card-muted">
              <Loader2 size={11} className="tool-spin-icon" /> Running…
            </div>
          )}

          {isError && (
            <div className="tool-call-error-banner" style={{ marginTop: output ? "var(--space-2)" : 0 }}>
              <AlertTriangle size={12} />
              <span>{state?.error || "Command failed"}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ── Edit / Write / MultiEdit Tool Card ────────────────────────────

export function isEditCard(name: string): boolean {
  const n = name.toLowerCase();
  return (
    !n.includes("neovim") &&
    !n.includes("todo") &&
    !n.includes("notebook") &&
    (n.includes("edit") || n.includes("write"))
  );
}

export function EditCard({ part }: { part: MessagePart }) {
  const toolName = part.tool || part.toolName || "unknown";
  const { shouldAutoOpen } = useAutoOpen();
  const [expanded, setExpanded] = useState(() => shouldAutoOpen(toolName));

  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const durationMs =
    state?.time?.start && state?.time?.end ? state.time.end - state.time.start : null;

  const input = useMemo(() => asObj(state?.input), [state?.input]);
  const filePath = str(input.filePath || input.file_path || input.path);

  const lname = toolName.toLowerCase();
  const label = lname.includes("write")
    ? "Write"
    : lname.includes("multiedit") || lname.includes("multi_edit")
    ? "MultiEdit"
    : "Edit";

  const toggle = () => setExpanded(e => !e);

  return (
    <div className={`tc-card tc-edit${isError ? " tool-call-error" : ""}`}>
      <button className="tc-card-head tc-card-head-btn" onClick={toggle}>
        <span className="tc-card-chevron">{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}</span>
        <Pencil size={12} className="tc-card-icon" />
        <span className="tc-card-label">{label}</span>
        {filePath && <FilePath path={filePath} />}
        <TcStatus status={status} durationMs={durationMs} />
      </button>
      {expanded && state?.input && (
        <div className="tc-card-body-flush">
          <EditDiffView input={state.input as Record<string, unknown>} />
        </div>
      )}
      {expanded && isError && (
        <div className="tc-card-body">
          <div className="tool-call-error-banner">
            <AlertTriangle size={12} />
            <span>{state?.error || "Edit failed"}</span>
          </div>
        </div>
      )}
    </div>
  );
}
