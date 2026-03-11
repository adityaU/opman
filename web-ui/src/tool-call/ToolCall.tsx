import React, { useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  Wrench,
  CheckCircle2,
  XCircle,
  Loader2,
  Clock,
} from "lucide-react";
import { SubagentSession } from "../SubagentSession";
import { ToolCallProps } from "./types";
import { formatToolName, formatDuration, getTaskSessionId } from "./helpers";
import { ToolInput, ToolOutput, TodoList, EditDiffView } from "./components";

export const ToolCall = React.memo(function ToolCall({
  part,
  childSession,
  subagentMessages,
  onOpenSession,
}: ToolCallProps) {
  const toolName = part.tool || part.toolName || "unknown";
  const shortName = formatToolName(toolName);

  const isTodoWrite = toolName.includes("todowrite") || toolName.includes("todo_write");
  const isTaskTool = toolName === "task";
  const isBashTool = toolName.includes("bash") || toolName.includes("shell") || toolName.includes("terminal");

  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isCompleted = status === "completed";
  const isRunning = status === "running" || status === "pending";
  const isEditTool = toolName.includes("edit") && !toolName.includes("neovim");

  const taskSessionId = isTaskTool ? getTaskSessionId(part, childSession) : null;
  const hasSubagentMessages = isTaskTool && taskSessionId
    ? (subagentMessages?.get(taskSessionId)?.length ?? 0) > 0
    : false;

  const [expanded, setExpanded] = useState(
    isTodoWrite || (isTaskTool && (isRunning || isCompleted || isError))
  );
  const [userToggled, setUserToggled] = useState(false);

  // Auto-expand running bash tools
  React.useEffect(() => {
    if (!userToggled && isBashTool && isRunning) setExpanded(true);
  }, [userToggled, isBashTool, isRunning]);

  // Auto-expand task tools when running or receiving messages
  React.useEffect(() => {
    if (!userToggled && isTaskTool && (isRunning || hasSubagentMessages)) setExpanded(true);
  }, [userToggled, isTaskTool, isRunning, hasSubagentMessages]);

  const handleToggle = () => {
    setUserToggled(true);
    setExpanded(!expanded);
  };

  const durationMs =
    state?.time?.start && state?.time?.end
      ? state.time.end - state.time.start
      : null;

  const inputData = state?.input;
  const hasInput =
    inputData != null &&
    (typeof inputData === "string"
      ? inputData.length > 0
      : Object.keys(inputData).length > 0);

  const outputData = state?.output;
  const hasOutput = outputData != null && outputData.length > 0;

  return (
    <div className={`tool-call ${isError ? "tool-call-error" : ""}`}>
      <button className="tool-call-header" onClick={handleToggle}>
        <span className="tool-call-icon">
          {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </span>
        <Wrench size={12} />
        <span className="tool-call-name">{shortName}</span>
        {state?.title && <span className="tool-call-title">{state.title}</span>}
        <span className="tool-call-status">
          {durationMs != null && (
            <span className="tool-call-duration">
              <Clock size={10} />
              {formatDuration(durationMs)}
            </span>
          )}
          {isCompleted ? (
            <CheckCircle2 size={12} className="tool-success-icon" />
          ) : isError ? (
            <XCircle size={12} className="tool-error-icon" />
          ) : isRunning ? (
            <span className="tool-call-pending">
              <Loader2 size={12} className="tool-spin-icon" /> running...
            </span>
          ) : (
            <span className="tool-call-pending">{status}</span>
          )}
        </span>
      </button>

      {expanded && (
        <div className="tool-call-body">
          {isTodoWrite && hasInput ? (
            <div className="tool-call-section">
              <div className="tool-call-section-label">Todos</div>
              <TodoList input={inputData!} />
            </div>
          ) : (
            <>
              {hasInput && !isTaskTool && (
                <div className="tool-call-section">
                  <div className="tool-call-section-label">Input</div>
                  {isEditTool ? (
                    <EditDiffView input={inputData!} />
                  ) : (
                    <ToolInput data={inputData!} />
                  )}
                </div>
              )}

              {isTaskTool && taskSessionId ? (
                <SubagentSession
                  sessionId={taskSessionId}
                  title={state?.title || childSession?.title || "Task"}
                  messages={subagentMessages?.get(taskSessionId)}
                  isRunning={isRunning}
                  isCompleted={isCompleted}
                  isError={isError}
                  onOpenSession={onOpenSession}
                />
              ) : (
                <>
                  {hasOutput && (
                    <div className="tool-call-section">
                      <div className="tool-call-section-label">Output</div>
                      {state?.metadata?.truncated && (
                        <span className="tool-call-truncated">[truncated] </span>
                      )}
                      <ToolOutput output={outputData!} toolName={toolName} isLive={isRunning} />
                    </div>
                  )}

                  {!hasOutput && isRunning && (
                    <div className="tool-call-section">
                      <div className="tool-call-section-label">Output</div>
                      <pre className="tool-call-pre tool-call-live-output">
                        <Loader2 size={12} className="tool-spin-icon" /> Waiting for output...
                      </pre>
                    </div>
                  )}
                </>
              )}
            </>
          )}

          {!isTodoWrite && !isTaskTool && !hasInput && !hasOutput && (
            <div className="tool-call-section">
              <pre className="tool-call-pre tool-call-empty">No data available</pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
});
