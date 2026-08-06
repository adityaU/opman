import React, { useState } from "react";
import {
  CheckCircle2,
  XCircle,
  Loader2,
  AlertTriangle,
} from "lucide-react";
import { SubagentSession } from "../SubagentSession";
import { BackgroundTask } from "./BackgroundTask";
import { ToolCallProps } from "./types";
import { getTaskSessionId } from "./helpers";
import { ToolOutput, TodoList } from "./components";
import { A2UIRenderer } from "./a2ui";
import { KanbanToolCard, isKanbanTool } from "./KanbanToolCard";
import { ReadCard, isReadTool, BashCard, isBashCard, EditCard, isEditCard } from "./readBashCards";
import { WebSearchCard, isWebSearchCard, WebFetchCard, isWebFetchCard, GlobCard, isGlobCard } from "./webGlobCards";
import { GenericToolCard } from "./GenericToolCard";
import { AgentManagerToolCard, isAgentManagerTool } from "./AgentManagerToolCard";
import { useAutoOpen } from "../hooks/useAutoOpen";

export const ToolCall = React.memo(function ToolCall({
  part,
  childSession,
  subagentMessages,
  onOpenSession,
}: ToolCallProps) {
  const toolName = part.tool || part.toolName || "unknown";
  const { shouldAutoOpen } = useAutoOpen();

  const lname = toolName.toLowerCase();
  const isTodoWrite = lname.includes("todowrite") || lname.includes("todo_write");
  const isTaskTool = toolName === "task";
  // A background task is a `run_in_background` Bash call, tagged by the backend. It is
  // distinct from a subagent ("task") and gets its own nested, tagged card. Fall back to
  // sniffing the launch ack in case the metadata tag is ever absent.
  const isBackgroundTask =
    part.state?.metadata?.background === true ||
    (typeof part.state?.output === "string" &&
      part.state.output.startsWith("Command running in background with ID:"));
  const isA2UI = lname.includes("ui_render") || lname.includes("ui_ui_render") || toolName === "a2ui";
  const isAgentManager = isAgentManagerTool(toolName);
  const isKanban = isKanbanTool(toolName);
  const isRead = !isKanban && isReadTool(toolName);
  const isBash = !isBackgroundTask && isBashCard(toolName);
  const isEdit = isEditCard(toolName);
  const isWebSearch = isWebSearchCard(toolName);
  const isWebFetch = isWebFetchCard(toolName);
  const isGlob = !isRead && isGlobCard(toolName);

  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isCompleted = status === "completed";
  const isRunning = status === "running" || status === "pending";
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

  // Auto-expand task tools when running or receiving messages
  React.useEffect(() => {
    if (!userToggled && isTaskTool && (isRunning || hasSubagentMessages)) setExpanded(true);
  }, [userToggled, isTaskTool, isRunning, hasSubagentMessages]);

  const handleToggle = () => {
    setUserToggled(true);
    setExpanded(!expanded);
  };

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

  // Agent-manager MCP tools render as routing/progress cards instead of generic JSON.
  if (isAgentManager) {
    return <AgentManagerToolCard part={part} />;
  }

  // Kanban MCP tools render as purpose-built cards — no accordion wrapper
  if (isKanban) {
    return <KanbanToolCard part={part} />;
  }

  // Read tool — compact file card
  if (isRead) {
    return <ReadCard part={part} />;
  }

  // Bash tool — terminal card (background already handled above)
  if (isBash) {
    return <BashCard part={part} />;
  }

  // Edit / Write / MultiEdit — diff card
  if (isEdit) {
    return <EditCard part={part} />;
  }

  // Web search — result list card
  if (isWebSearch) {
    return <WebSearchCard part={part} />;
  }

  // Web fetch — URL + content card
  if (isWebFetch) {
    return <WebFetchCard part={part} />;
  }

  // Glob / Grep / LS — file list card
  if (isGlob) {
    return <GlobCard part={part} />;
  }

  // Task tools render directly without accordion wrapper
  if (isTaskTool) {
    return (
      <div className={`tool-call tool-call-task-inline ${isError ? "tool-call-error" : ""}`}>
        {taskSessionId ? (
          <SubagentSession
            sessionId={taskSessionId}
            title={childSession?.title || "Task"}
            progressTitle={state?.title}
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

  // TodoWrite — accordion with checklist view
  if (isTodoWrite) {
    return (
      <div className={`tool-call ${isError ? "tool-call-error" : ""}`}>
        <button className="tool-call-header" onClick={handleToggle}>
          <span className="tool-call-icon">
            {expanded ? <CheckCircle2 size={12} className="tool-success-icon" /> : <CheckCircle2 size={12} />}
          </span>
          <span className="tool-call-name">TodoWrite</span>
          <span className="tool-call-status">
            {isCompleted ? (
              <CheckCircle2 size={12} className="tool-success-icon" />
            ) : isError ? (
              <XCircle size={12} className="tool-error-icon" />
            ) : (
              <Loader2 size={12} className="tool-spin-icon" />
            )}
          </span>
        </button>
        {expanded && hasInput && (
          <div className="tool-call-body">
            <TodoList input={inputData!} />
          </div>
        )}
      </div>
    );
  }

  // Everything else — generic MCP tool card (no raw JSON)
  return <GenericToolCard part={part} />;
});
