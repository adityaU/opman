import { useMemo, useRef } from "react";
import type { ModalName } from "./useModalState";
import type { PersonalMemoryItem, ImageAttachment } from "../api";
import type { Message } from "../types";
import {
  createHandleSend, createHandleAbort, createHandleAgentChange,
  createHandleCommand, createHandlePermissionReply, createHandleQuestionReply,
  createHandleQuestionDismiss,
  createHandleSelectSession, createHandleNewSession, createHandleSwitchProject,
  createHandleModelSelected,
} from "../chatLayoutHandlers";
import type { HandlerDeps } from "../chatLayoutHandlers";

/* ── Input types ───────────────────────────────────────── */

export interface ChatHandlerInputs {
  activeSessionId: string | null;
  appState: any;
  selectedModel: any;
  selectedAgent: string;
  sending: boolean;
  activeMemoryItems: PersonalMemoryItem[];
  setSending: (v: boolean, sessionId?: string) => void;
  setSelectedModel: (m: any) => void;
  setSelectedAgent: (a: string) => void;
  setMobileInputHidden: (v: boolean) => void;
  addToast: (msg: string, type: "success" | "error" | "info" | "warning") => void;
  addOptimisticMessage: (text: string, images?: ImageAttachment[]) => void;
  refreshState: () => void;
  clearPermission: (id: string) => void;
  clearQuestion: (id: string) => void;
  setMobileSidebarOpen: (v: boolean) => void;
  closeMobileSidebarSilent: () => void;
  /** Navigate to a session via URL (single source of truth). */
  setUrlSession: (sessionId: string, projectIdx: number) => void;
  openModal: (name: string) => void;
  toggleSidebar: () => void;
  toggleTerminal: () => void;
  toggleNeovim: () => void;
  toggleGit: () => void;
  toggleDebug: () => void;
  toggleSplitView: () => void;
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

  // Keep sending in a ref — handlers read at call-time so session switches
  // don't stale-close over the wrong session's sending flag.
  const sendingRef = useRef(inputs.sending);
  sendingRef.current = inputs.sending;

  // Keep activeMemoryItems in a ref — handlers read at call-time so freshly
  // fetched memories are always used, even during the brief re-render window.
  const memoryRef = useRef(inputs.activeMemoryItems);
  memoryRef.current = inputs.activeMemoryItems;

  // Keep messages in a ref so /copy reads current messages without memo invalidation.
  const messagesRef = useRef(inputs.getMessages);
  messagesRef.current = inputs.getMessages;

  const deps: HandlerDeps = useMemo(() => ({
    // Provide a getter that reads the ref at call-time
    get activeSessionId() { return sessionIdRef.current; },
    appState: inputs.appState,
    selectedModel: inputs.selectedModel,
    selectedAgent: inputs.selectedAgent,
    get sending() { return sendingRef.current; },
    get activeMemoryItems() { return memoryRef.current; },
    setSending: inputs.setSending,
    setSelectedModel: inputs.setSelectedModel,
    setSelectedAgent: inputs.setSelectedAgent,
    setMobileInputHidden: inputs.setMobileInputHidden,
    addToast: inputs.addToast,
    addOptimisticMessage: inputs.addOptimisticMessage,
    refreshState: inputs.refreshState,
    clearPermission: inputs.clearPermission,
    clearQuestion: inputs.clearQuestion,
    setMobileSidebarOpen: inputs.setMobileSidebarOpen,
    closeMobileSidebarSilent: inputs.closeMobileSidebarSilent,
    setUrlSession: inputs.setUrlSession,
    openModal: inputs.openModal,
    toggleSidebar: inputs.toggleSidebar,
    toggleTerminal: inputs.toggleTerminal,
    toggleNeovim: inputs.toggleNeovim,
    toggleGit: inputs.toggleGit,
    toggleDebug: inputs.toggleDebug,
    toggleSplitView: inputs.toggleSplitView,
    getMessages: () => messagesRef.current(),
  }), [
    // activeSessionId, sending, activeMemoryItems intentionally omitted — read from refs via getters
    inputs.appState, inputs.selectedModel,
    inputs.selectedAgent,
    inputs.setSending, inputs.setSelectedModel, inputs.setSelectedAgent,
    inputs.setMobileInputHidden, inputs.addToast, inputs.addOptimisticMessage,
    inputs.refreshState, inputs.clearPermission, inputs.clearQuestion,
    inputs.setMobileSidebarOpen, inputs.closeMobileSidebarSilent, inputs.setUrlSession,
    inputs.openModal, inputs.toggleSidebar, inputs.toggleTerminal, inputs.toggleNeovim,
    inputs.toggleGit, inputs.toggleDebug, inputs.toggleSplitView,
  ]);

  const handleSend = useMemo(() => createHandleSend(deps), [deps]);
  const handleAbort = useMemo(() => createHandleAbort(deps), [deps]);
  const handleAgentChange = useMemo(() => createHandleAgentChange(deps), [deps]);
  const handleCommand = useMemo(() => createHandleCommand(deps), [deps]);
  const handlePermissionReply = useMemo(() => createHandlePermissionReply(deps), [deps]);
  const handleQuestionReply = useMemo(() => createHandleQuestionReply(deps), [deps]);
  const handleQuestionDismiss = useMemo(() => createHandleQuestionDismiss(deps), [deps]);
  const handleSelectSession = useMemo(() => createHandleSelectSession(deps), [deps]);
  const handleNewSession = useMemo(() => createHandleNewSession(deps), [deps]);
  const handleSwitchProject = useMemo(() => createHandleSwitchProject(deps), [deps]);
  const handleModelSelected = useMemo(() => createHandleModelSelected(deps), [deps]);

  return {
    handleSend, handleAbort, handleAgentChange, handleCommand,
    handlePermissionReply, handleQuestionReply, handleQuestionDismiss,
    handleSelectSession,
    handleNewSession, handleSwitchProject, handleModelSelected,
  };
}
