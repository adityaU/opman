import React, { useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  Wrench,
  CheckCircle2,
  XCircle,
  Loader2,
  Clock,
  AlertTriangle,
} from "lucide-react";
import { SubagentSession } from "../SubagentSession";
import { BackgroundTask } from "./BackgroundTask";
import { ToolCallProps } from "./types";
import { formatToolName, formatDuration, getTaskSessionId } from "./helpers";
import { ToolInput, ToolOutput, TodoList, EditDiffView } from "./components";
import { A2UIRenderer } from "./a2ui";
import { KanbanToolCard, isKanbanTool } from "./KanbanToolCard";
import { useAutoOpen } from "../hooks/useAutoOpen";

export const ToolCall = React.memo(function ToolCall({
  part,
  childSession,
  subagentMessages,
  onOpenSession,
}: ToolCallProps) {
  const toolName = part.tool || part.toolName || "unknown";
  const shortName = formatToolName(toolName);
  const { shouldAutoOpen } = useAutoOpen();

  const isTodoWrite = toolName.includes("todowrite") || toolName.includes("todo_write");
  const isTaskTool = toolName === "task";
  // A background task is a `run_in_background` Bash call, tagged by the backend. It is
  // distinct from a subagent ("task") and gets its own nested, tagged card. Fall back to
  // sniffing the launch ack in case the metadata tag is ever absent.
  const isBackgroundTask =
    part.state?.metadata?.background === true ||
    (typeof part.state?.output === "string" &&
      part.state.output.startsWith("Command running in background with ID:"));
  const isBashTool = toolName.includes("bash") || toolName.includes("shell") || toolName.includes("terminal");
  const isA2UI = toolName.includes("ui_render") || toolName.includes("ui_ui_render") || toolName === "a2ui";
  const isKanban = isKanbanTool(toolName);

  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isCompleted = status === "completed";
  const isRunning = status === "running" || status === "pending";
  // Render a file diff for edit/write tools across all engines. Match case-INsensitively:
  // claude/claudep send capitalized names (`Edit`, `Write`, `MultiEdit`, `NotebookEdit`),
  // opencode sends lowercase (`edit`, `write`). Exclude neovim edits and TodoWrite (which
  // has its own renderer).
  const lname = toolName.toLowerCase();
  const isEditTool =
    !lname.includes("neovim") &&
    !lname.includes("todo") &&
    (lname.includes("edit") || lname.includes("write"));

  const taskSessionId = isTaskTool ? getTaskSessionId(part, childSession) : null;
  const hasSubagentMessages = isTaskTool && taskSessionId
    ? (subagentMessages?.get(taskSessionId)?.length ?? 0) > 0
    : false;

  const [expanded, setExpanded] = useState(
    isTodoWrite
    || (isTaskTool && (isRunning || isCompleted || isError))
    || shouldAutoOpen(toolName)
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

  // Use final output when available; fall back to live metadata.output while running
  const finalOutput = state?.output;
  const liveOutput = typeof state?.metadata?.output === "string" ? state.metadata.output : null;
  const outputData = (finalOutput && finalOutput.length > 0) ? finalOutput : liveOutput;
  const hasOutput = outputData != null && outputData.length > 0;

  // Extract error text for display when tool errored
  const errorText = isError
    ? state?.error || (hasOutput ? null : "Tool call failed")
    : null;

  // Background tasks render as their own tagged, nested card (not the generic accordion).
  if (isBackgroundTask) {
    return <BackgroundTask part={part} />;
  }

  // A2UI renders directly — no accordion wrapper
  if (isA2UI && hasInput) {
    return <A2UIRenderer input={inputData} />;
  }
  if (isA2UI && isRunning) {
    return (
      <div className="a2ui-loading">
        <span className="tool-pulse-dot" />
        Rendering...
      </div>
    );
  }

  // Kanban MCP tools render as purpose-built cards — no accordion wrapper
  if (isKanban) {
    return <KanbanToolCard part={part} />;
  }

  // Task tools render directly without accordion wrapper
  if (isTaskTool) {
    return (
      <div className={`tool-call tool-call-task-inline ${isError ? "tool-call-error" : ""}`}>
        {taskSessionId ? (
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
              <div className="tool-call-body">
                <div className="tool-call-section">
                  <div className="tool-call-section-label">Output</div>
                  {state?.metadata?.truncated && (
                    <span className="tool-call-truncated">[truncated] </span>
                  )}
                  <ToolOutput output={outputData!} toolName={toolName} isLive={isRunning} />
                </div>
              </div>
            )}
            {errorText && (
              <div className="tool-call-body">
                <div className="tool-call-section">
                  <div className="tool-call-error-banner">
                    <AlertTriangle size={12} />
                    <span>{errorText}</span>
                  </div>
                </div>
              </div>
            )}
            {!hasOutput && !errorText && isRunning && (
              <div className="tool-call-body">
                <div className="tool-call-section">
                  <div className="tool-call-section-label">Output</div>
                  <pre className="tool-call-pre tool-call-live-output">
                    <Loader2 size={12} className="tool-spin-icon" /> Waiting for output...
                  </pre>
                </div>
              </div>
            )}
          </>
        )}
      </div>
    );
  }

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

                    {errorText && (
                      <div className="tool-call-section">
                        <div className="tool-call-error-banner">
                          <AlertTriangle size={12} />
                          <span>{errorText}</span>
                        </div>
                      </div>
                    )}

                    {!hasOutput && !errorText && isRunning && (
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
