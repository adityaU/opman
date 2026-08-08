import { useMemo, useRef } from "react";
import type { ModalName } from "./useModalState";
import type { ImageAttachment } from "../api";
import type { Message } from "../types";
import {
  createHandleSend, createHandleAbort, createHandleAgentChange,
  createHandleCommand, createHandlePermissionReply, createHandleQuestionReply,
  createHandleQuestionDismiss, createHandleCopyTranscript,
} from "../chatLayoutHandlers";
import type { HandlerDeps } from "../chatLayoutHandlers";
import {
  createHandleSelectSession, createHandleNewSession, createHandleSwitchProject,
  createHandleModelSelected,
} from "../chatSessionHandlers";

/* ── Input types ───────────────────────────────────────── */

export interface ChatHandlerInputs {
  activeSessionId: string | null;
  /** URL-derived active project index — sole source of truth. */
  activeProjectIndex: number;
  appState: any;
  selectedModel: any;
  selectedAgent: string;
  /** Runner a session created right now should be created with. */
  runnerForNewSession: string;
  /** Explicit pick that disagrees with the active session's runner (handoff intent). */
  runnerSwitch: string | null;
  selectedEffort: string | null;
  selectedPermission: string;
  setSending: (v: boolean, sessionId?: string) => void;
  setSelectedModel: (m: any) => void;
  setSelectedAgent: (a: string) => void;
  clearRunnerChoice: () => void;
  /** Re-anchor the runner pick to a session that was just created or handed off. */
  bindRunnerChoice: (sessionId: string, runner: string) => void;
  setMobileInputHidden: (v: boolean) => void;
  addToast: (msg: string, type: "success" | "error" | "info" | "warning") => void;
  addOptimisticMessage: (text: string, images?: ImageAttachment[], sessionId?: string | null) => string | null;
  clearOptimistic: (sessionId?: string | null, id?: string) => void;
  refreshState: () => void;
  refreshMessages: (sessionId?: string | null, options?: { adoptView?: boolean }) => Promise<void>;
  clearPermission: (id: string) => void;
  clearQuestion: (id: string) => void;
  closeMobileSidebarSilent: () => void;
  /** Navigate to a session via URL (single source of truth). */
  setUrlSession: (sessionId: string | null, projectIdx: number) => void;
  /** Temporarily block background SSE-driven session adoption for non-switch actions. */
  blockSessionAdoption: (ms?: number) => void;
  /** Run a registered command by id — how a `/name` reaches its implementation. */
  runCommandId: (id: string) => boolean;
  /** Read current messages at call-time (avoids including in memo deps). */
  getMessages: () => Message[];
}

/* ── Hook ──────────────────────────────────────────────── */

/**
 * Builds all chat handler closures. Uses a ref for `activeSessionId` so that
 * session switches don't recreate every handler (the closures read the ref at
 * call-time). All other inputs are included in the deps memo normally.
 */
export function useChatHandlers(inputs: ChatHandlerInputs) {
  // Keep activeSessionId in a ref — handlers read it at call-time.
  // This prevents all 11 handlers from being recreated on every session switch.
  const sessionIdRef = useRef(inputs.activeSessionId);
  sessionIdRef.current = inputs.activeSessionId;

  // Keep activeProjectIndex in a ref — read at call-time like activeSessionId.
  const projectIdxRef = useRef(inputs.activeProjectIndex);
  projectIdxRef.current = inputs.activeProjectIndex;

  // Keep messages in a ref so /copy reads current messages without memo invalidation.
  const messagesRef = useRef(inputs.getMessages);
  messagesRef.current = inputs.getMessages;

  const deps: HandlerDeps = useMemo(() => ({
    // Provide a getter that reads the ref at call-time
    get activeSessionId() { return sessionIdRef.current; },
    get activeProjectIndex() { return projectIdxRef.current; },
    appState: inputs.appState,
    selectedModel: inputs.selectedModel,
    selectedAgent: inputs.selectedAgent,
    runnerForNewSession: inputs.runnerForNewSession,
    runnerSwitch: inputs.runnerSwitch,
    selectedEffort: inputs.selectedEffort,
    selectedPermission: inputs.selectedPermission,
    setSending: inputs.setSending,
    setSelectedModel: inputs.setSelectedModel,
    setSelectedAgent: inputs.setSelectedAgent,
    clearRunnerChoice: inputs.clearRunnerChoice,
    bindRunnerChoice: inputs.bindRunnerChoice,
    setMobileInputHidden: inputs.setMobileInputHidden,
    addToast: inputs.addToast,
    addOptimisticMessage: inputs.addOptimisticMessage,
    clearOptimistic: inputs.clearOptimistic,
    refreshState: inputs.refreshState,
    refreshMessages: inputs.refreshMessages,
    clearPermission: inputs.clearPermission,
    clearQuestion: inputs.clearQuestion,
    closeMobileSidebarSilent: inputs.closeMobileSidebarSilent,
    setUrlSession: inputs.setUrlSession,
    blockSessionAdoption: inputs.blockSessionAdoption,
    runCommandId: inputs.runCommandId,
    getMessages: () => messagesRef.current(),
  }), [
    // activeSessionId and activeProjectIndex intentionally omitted — read from refs via getters
    inputs.appState, inputs.selectedModel,
    inputs.selectedAgent, inputs.runnerForNewSession, inputs.runnerSwitch, inputs.selectedEffort, inputs.selectedPermission,
    inputs.setSending, inputs.setSelectedModel, inputs.setSelectedAgent, inputs.clearRunnerChoice, inputs.bindRunnerChoice,
    inputs.setMobileInputHidden, inputs.addToast, inputs.addOptimisticMessage,
    inputs.clearOptimistic, inputs.refreshState, inputs.refreshMessages, inputs.clearPermission, inputs.clearQuestion,
    inputs.closeMobileSidebarSilent, inputs.setUrlSession,
    inputs.blockSessionAdoption, inputs.runCommandId,
  ]);

  const handleSend = useMemo(() => createHandleSend(deps), [deps]);
  const handleAbort = useMemo(() => createHandleAbort(deps), [deps]);
  const handleAgentChange = useMemo(() => createHandleAgentChange(deps), [deps]);
  const handleCommand = useMemo(() => createHandleCommand(deps), [deps]);
  const handleCopyTranscript = useMemo(() => createHandleCopyTranscript(deps), [deps]);
  const handlePermissionReply = useMemo(() => createHandlePermissionReply(deps), [deps]);
  const handleQuestionReply = useMemo(() => createHandleQuestionReply(deps), [deps]);
  const handleQuestionDismiss = useMemo(() => createHandleQuestionDismiss(deps), [deps]);
  const handleSelectSession = useMemo(() => createHandleSelectSession(deps), [deps]);
  const handleNewSession = useMemo(() => createHandleNewSession(deps), [deps]);
  const handleSwitchProject = useMemo(() => createHandleSwitchProject(deps), [deps]);
  const handleModelSelected = useMemo(() => createHandleModelSelected(deps), [deps]);

  return {
    handleSend, handleAbort, handleAgentChange, handleCommand, handleCopyTranscript,
    handlePermissionReply, handleQuestionReply, handleQuestionDismiss,
    handleSelectSession,
    handleNewSession, handleSwitchProject, handleModelSelected,
  };
}
