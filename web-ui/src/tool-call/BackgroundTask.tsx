import React, { useEffect, useRef, useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  Terminal,
  Loader2,
  CheckCircle2,
  XCircle,
  Clock,
} from "lucide-react";
import type { MessagePart } from "../types";
import { formatDuration } from "./helpers";
import { ToolOutput } from "./components";

interface BackgroundTaskProps {
  part: MessagePart;
}

/**
 * Renders a Claude background task (`Bash` with `run_in_background: true`) as a
 * distinct, tagged card nested inside the main turn.
 *
 * Background tasks differ from subtasks/subagents: there is no child session, just
 * a detached shell command whose output streams to a file. The card is tagged
 * "BACKGROUND" to set it apart from the subagent ("task") cards, and tracks the
 * task across its real lifecycle: it stays *running* after the launch ack and only
 * resolves to completed/failed when the matching `<task-notification>` arrives
 * (folded into this part by the backend).
 */
export const BackgroundTask = React.memo(function BackgroundTask({ part }: BackgroundTaskProps) {
  const state = part.state;
  const status = state?.status || "running";
  const isError = status === "error";
  const isCompleted = status === "completed";
  const isRunning = !isError && !isCompleted;

  const meta = state?.metadata;
  const input = state?.input;
  const command =
    input && typeof input === "object" && !Array.isArray(input)
      ? ((input as Record<string, unknown>).command as string | undefined)
      : typeof input === "string"
        ? input
        : undefined;
  const taskId = meta?.taskId;
  const summary = meta?.summary;
  // Live/final command output streamed from the task's output file by the backend.
  const output = meta?.output || state?.output;
  const hasOutput = typeof output === "string" && output.length > 0;

  const durationMs =
    state?.time?.start && state?.time?.end ? state.time.end - state.time.start : null;

  // Expanded while running, auto-collapse once finished (user can override).
  const [expanded, setExpanded] = useState(isRunning);
  const [userToggled, setUserToggled] = useState(false);
  useEffect(() => {
    if (userToggled) return;
    setExpanded(isRunning);
  }, [isRunning, userToggled]);

  const handleToggle = () => {
    setUserToggled(true);
    setExpanded((e) => !e);
  };

  // Auto-scroll the output tail while running.
  const outRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (isRunning && expanded && outRef.current) {
      const el = outRef.current;
      if (el.scrollHeight - el.scrollTop - el.clientHeight < 120) {
        el.scrollTop = el.scrollHeight;
      }
    }
  }, [output, isRunning, expanded]);

  return (
    <div
      className={`background-task${isRunning ? " background-running" : ""}${
        isError ? " background-error-state" : ""
      }`}
    >
      <button className="background-task-header" onClick={handleToggle} type="button" aria-expanded={expanded}>
        <span className="background-task-chevron">
          {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </span>
        <Terminal size={13} className="background-task-icon" />
        <span className="background-task-tag">BACKGROUND</span>
        <span className="background-task-title">Background task</span>
        <span className="background-task-status">
          {durationMs != null && (
            <span className="background-task-duration">
              <Clock size={10} />
              {formatDuration(durationMs)}
            </span>
          )}
          {isRunning ? (
            <>
              <span className="background-live-dot" />
              <Loader2 size={11} className="tool-spin-icon" />
              <span className="background-task-status-text">running</span>
            </>
          ) : isCompleted ? (
            <>
              <CheckCircle2 size={12} className="tool-success-icon" />
              <span className="background-task-status-text">completed</span>
            </>
          ) : (
            <>
              <XCircle size={12} className="tool-error-icon" />
              <span className="background-task-status-text">failed</span>
            </>
          )}
        </span>
      </button>

      {expanded && (
        <div className="background-task-body">
          {command && (
            <div className="background-task-command">
              <span className="background-task-prompt-char">$</span>
              <code>{command}</code>
            </div>
          )}
          {taskId && <div className="background-task-meta">id: {taskId}</div>}

          {hasOutput ? (
            <div className="background-task-output" ref={outRef}>
              <ToolOutput output={output!} toolName="bash" isLive={isRunning} />
            </div>
          ) : isRunning ? (
            <div className="background-task-waiting">
              <Loader2 size={12} className="tool-spin-icon" /> Waiting for output…
            </div>
          ) : (
            <div className="background-task-waiting background-task-no-data">No output captured.</div>
          )}

          {summary && (
            <div className={`background-task-summary${isError ? " is-error" : ""}`}>{summary}</div>
          )}
        </div>
      )}
    </div>
  );
});
