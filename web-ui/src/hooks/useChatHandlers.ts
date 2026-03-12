import { useMemo } from "react";
import type { ModalName } from "./useModalState";
import type { PersonalMemoryItem } from "../api";
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
  setSending: (v: boolean) => void;
  setSelectedModel: (m: any) => void;
  setSelectedAgent: (a: string) => void;
  setMobileInputHidden: (v: boolean) => void;
  addToast: (msg: string, type: "success" | "error" | "info" | "warning") => void;
  addOptimisticMessage: (text: string) => void;
  refreshState: () => void;
  clearPermission: (id: string) => void;
  clearQuestion: (id: string) => void;
  setMobileSidebarOpen: (v: boolean) => void;
  openModal: (name: string) => void;
  toggleSidebar: () => void;
  toggleTerminal: () => void;
  toggleNeovim: () => void;
  toggleGit: () => void;
  toggleSplitView: () => void;
}

/* ── Hook ──────────────────────────────────────────────── */

export function useChatHandlers(inputs: ChatHandlerInputs) {
  const deps: HandlerDeps = useMemo(() => inputs, [
    inputs.activeSessionId, inputs.appState, inputs.selectedModel,
    inputs.selectedAgent, inputs.sending, inputs.activeMemoryItems,
    inputs.setSending, inputs.setSelectedModel, inputs.setSelectedAgent,
    inputs.setMobileInputHidden, inputs.addToast, inputs.addOptimisticMessage,
    inputs.refreshState, inputs.clearPermission, inputs.clearQuestion,
    inputs.setMobileSidebarOpen, inputs.openModal,
    inputs.toggleSidebar, inputs.toggleTerminal, inputs.toggleNeovim,
    inputs.toggleGit, inputs.toggleSplitView,
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
