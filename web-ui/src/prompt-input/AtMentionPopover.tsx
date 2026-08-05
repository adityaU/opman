/**
 * AtMentionPopover — the @-mention surface: agents and files in one list.
 *
 * Extracted from `components.tsx` when it gained its own keyboard handling;
 * that file is a bag of small presentational pieces and this is no longer one.
 */
import React from "react";
import { AtSign, File, Folder, Loader2 } from "lucide-react";
import type { AgentInfo, FileSearchEntry } from "../api";
import { agentColor } from "./helpers";


interface AtMentionPopoverProps {
  agents: AgentInfo[];
  fileResults: FileSearchEntry[];
  fileLoading: boolean;
  popoverRef: React.RefObject<HTMLDivElement>;
  onSelectAgent: (agentId: string) => void;
  onSelectFile: (entry: FileSearchEntry) => void;
  onClose?: () => void;
}

/**
 * Agents and files in one list.
 *
 * Both sections used to render with the same row classes, so the only thing
 * telling an agent from a file was the leading glyph, and a file's path — the
 * part that disambiguates two files with the same name — was clipped at its
 * end, which is where the meaning is. It also had no keyboard navigation at
 * all: the popover opened while the caret was in the textarea, and arrow keys
 * went to the textarea. It is a listbox now, and it answers the keys.
 */
export function AtMentionPopover({
  agents, fileResults, fileLoading, popoverRef, onSelectAgent, onSelectFile, onClose,
}: AtMentionPopoverProps) {
  const hasAgents = agents.length > 0;
  const hasFiles = fileResults.length > 0;
  const empty = !hasAgents && !hasFiles && !fileLoading;

  // One cursor across both sections: a flat list is what the keys move through,
  // whatever the headings suggest.
  const rows = React.useMemo(() => [
    ...agents.map((agent) => ({ kind: "agent" as const, agent })),
    ...fileResults.map((entry) => ({ kind: "file" as const, entry })),
  ], [agents, fileResults]);
  const [cursor, setCursor] = React.useState(0);
  React.useEffect(() => { setCursor(0); }, [rows.length]);

  const choose = React.useCallback((index: number) => {
    const row = rows[index];
    if (!row) return;
    if (row.kind === "agent") onSelectAgent(row.agent.id);
    else onSelectFile(row.entry);
  }, [rows, onSelectAgent, onSelectFile]);

  React.useEffect(() => {
    if (rows.length === 0) return;
    // Capture phase: the caret is in the textarea, which would otherwise move
    // on the same arrow keys.
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setCursor((c) => Math.min(rows.length - 1, c + 1));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setCursor((c) => Math.max(0, c - 1));
      } else if (event.key === "Enter" || event.key === "Tab") {
        event.preventDefault();
        event.stopPropagation();
        choose(cursor);
      } else if (event.key === "Escape") {
        event.preventDefault();
        onClose?.();
      }
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, [rows, cursor, choose, onClose]);

  return (
    <div
      className="prompt-at-popover composer-popover"
      ref={popoverRef}
      role="listbox"
      aria-label="Agents and files"
    >
      {hasAgents && (
        <>
          <div className="composer-popover-group"><AtSign size={11} /> Agents</div>
          {agents.map((agent, index) => {
            const color = agentColor(agent.id, agent.color);
            return (
              <button
                key={agent.id}
                type="button"
                role="option"
                aria-selected={index === cursor}
                className={`composer-popover-row${index === cursor ? " is-cursor" : ""}`}
                onMouseEnter={() => setCursor(index)}
                onClick={() => onSelectAgent(agent.id)}
              >
                <span className="composer-popover-icon">
                  {color
                    ? <span className="prompt-agent-dot" style={{ backgroundColor: color }} />
                    : <AtSign size={12} />}
                </span>
                <span className="composer-popover-label">{agent.label}</span>
                <span className="composer-popover-detail">{agent.description}</span>
              </button>
            );
          })}
        </>
      )}
      {(hasFiles || fileLoading) && (
        <>
          <div className="composer-popover-group"><File size={11} /> Files</div>
          {fileResults.map((entry, fileIndex) => {
            const index = agents.length + fileIndex;
            return (
              <button
                key={entry.path}
                type="button"
                role="option"
                aria-selected={index === cursor}
                className={`composer-popover-row${index === cursor ? " is-cursor" : ""}`}
                onMouseEnter={() => setCursor(index)}
                onClick={() => onSelectFile(entry)}
              >
                <span className="composer-popover-icon">
                  {entry.is_dir ? <Folder size={12} /> : <File size={12} />}
                </span>
                <span className="composer-popover-label">{entry.name}</span>
                <span className="composer-popover-detail is-path" title={entry.path}>
                  <span>{entry.path}</span>
                </span>
              </button>
            );
          })}
          {fileLoading && !hasFiles && (
            <div className="composer-popover-loading">
              <Loader2 size={12} className="spinning" />
              <span>Searching files…</span>
            </div>
          )}
        </>
      )}
      {empty && <div className="composer-popover-empty">No agents or files match</div>}
    </div>
  );
}
