import { useState, useCallback, useRef, useEffect } from "react";

export type ModalName =
  | "commandPalette" | "modelPicker" | "agentPicker" | "themeSelector"
  | "cheatsheet" | "todoPanel" | "sessionSelector" | "contextInput"
  | "settings" | "watcher" | "contextWindow" | "diffReview"
  | "searchBar" | "crossSearch" | "splitView" | "sessionGraph"
  | "sessionDashboard" | "activityFeed" | "notificationPrefs"
  | "assistantCenter" | "inbox" | "memory" | "autonomy"
  | "routines" | "delegation" | "missions" | "workspaceManager"
  | "addProject" | "systemMonitor";

/** Escape-key dismiss priority, highest first. */
const ESCAPE_PRIORITY: ModalName[] = [
  "commandPalette", "modelPicker", "agentPicker", "themeSelector",
  "cheatsheet", "todoPanel", "sessionSelector", "contextInput",
  "settings", "watcher", "contextWindow", "diffReview",
  "searchBar", "crossSearch", "activityFeed", "notificationPrefs",
  "assistantCenter", "inbox", "memory", "autonomy",
  "routines", "delegation", "missions", "workspaceManager",
  "addProject", "systemMonitor",
];

type ModalOpenState = Record<ModalName, boolean>;

const INITIAL_STATE: ModalOpenState = Object.fromEntries(
  ESCAPE_PRIORITY.concat("splitView", "sessionGraph", "sessionDashboard")
    .map((k) => [k, false]),
) as ModalOpenState;

/** Sentinel on history.state to distinguish modal-pushed entries. */
const MODAL_HISTORY_KEY = "_modalLayer";

export interface ModalStateAPI {
  /** Check whether a modal is currently open. */
  isOpen: (name: ModalName) => boolean;
  /** Open a modal by name. */
  open: (name: ModalName) => void;
  /** Close a modal by name (with side-effect cleanup for searchBar/splitView). */
  close: (name: ModalName) => void;
  /** Toggle a modal by name. */
  toggle: (name: ModalName) => void;
  /** Close the highest-priority open modal. Returns true if one was closed. */
  closeTopModal: () => boolean;
  /** Full open-state record (for reading in JSX). */
  modals: ModalOpenState;
  /* Search auxiliary state */
  searchMatchIds: Set<string>;
  setSearchMatchIds: React.Dispatch<React.SetStateAction<Set<string>>>;
  activeSearchMatchId: string | null;
  setActiveSearchMatchId: React.Dispatch<React.SetStateAction<string | null>>;
  /* Split-view auxiliary state */
  splitViewSecondaryId: string | null;
  setSplitViewSecondaryId: React.Dispatch<React.SetStateAction<string | null>>;
}

export function useModalState(): ModalStateAPI {
  const [modals, setModals] = useState<ModalOpenState>(INITIAL_STATE);
  const [searchMatchIds, setSearchMatchIds] = useState<Set<string>>(new Set());
  const [activeSearchMatchId, setActiveSearchMatchId] = useState<string | null>(null);
  const [splitViewSecondaryId, setSplitViewSecondaryId] = useState<string | null>(null);

  // Keep a ref so closeTopModal doesn't depend on `modals` (avoids stale closure).
  const modalsRef = useRef(modals);
  modalsRef.current = modals;

  /** How many history entries we have pushed for currently-open modals. */
  const historyDepthRef = useRef(0);
  /** Guard: true while we are programmatically navigating history (history.back()). */
  const suppressPopRef = useRef(false);

  const isOpen = useCallback((name: ModalName) => modalsRef.current[name], []);

  // ── Internal helpers (no history side-effects) ──────────────────

  const closeRaw = useCallback((name: ModalName) => {
    setModals((prev) => (prev[name] ? { ...prev, [name]: false } : prev));
  }, []);

  const cleanupSideEffects = useCallback((name: ModalName) => {
    if (name === "searchBar") {
      setSearchMatchIds(new Set());
      setActiveSearchMatchId(null);
    } else if (name === "splitView") {
      setSplitViewSecondaryId(null);
    }
  }, []);

  // ── Public API ──────────────────────────────────────────────────

  const open = useCallback((name: ModalName) => {
    setModals((prev) => {
      if (prev[name]) return prev; // already open
      // Push a history entry so the back gesture closes this modal
      window.history.pushState({ [MODAL_HISTORY_KEY]: true }, "");
      historyDepthRef.current += 1;
      return { ...prev, [name]: true };
    });
  }, []);

  const close = useCallback((name: ModalName) => {
    setModals((prev) => {
      if (!prev[name]) return prev; // already closed
      // Pop the matching history entry we pushed on open
      if (historyDepthRef.current > 0) {
        historyDepthRef.current -= 1;
        suppressPopRef.current = true;
        window.history.back();
      }
      return { ...prev, [name]: false };
    });
    cleanupSideEffects(name);
  }, [cleanupSideEffects]);

  const toggle = useCallback((name: ModalName) => {
    if (modalsRef.current[name]) {
      close(name);
    } else {
      open(name);
    }
  }, [open, close]);

  const closeTopModal = useCallback((): boolean => {
    const cur = modalsRef.current;
    for (const name of ESCAPE_PRIORITY) {
      if (cur[name]) {
        close(name);
        return true;
      }
    }
    return false;
  }, [close]);

  // ── Back-gesture / popstate listener ────────────────────────────
  useEffect(() => {
    const handler = (e: PopStateEvent) => {
      // If we triggered this popstate ourselves (via history.back() in close()),
      // just consume the event — the modal is already closed.
      if (suppressPopRef.current) {
        suppressPopRef.current = false;
        return;
      }

      // Only handle popstate events that correspond to our modal history entries.
      // Mobile-overlay entries (_mobileOverlay) are handled by useMobileState.
      // Session/panel entries are handled by useUrlState.
      // We check historyDepthRef rather than e.state because the *popped-to*
      // state may not carry our sentinel (the sentinel was on the entry we left).
      if (historyDepthRef.current > 0) {
        historyDepthRef.current -= 1;
        // Close the highest-priority open modal without touching history
        // (the browser already consumed the history entry).
        const cur = modalsRef.current;
        for (const name of ESCAPE_PRIORITY) {
          if (cur[name]) {
            closeRaw(name);
            cleanupSideEffects(name);
            break;
          }
        }
      }
    };
    window.addEventListener("popstate", handler);
    return () => window.removeEventListener("popstate", handler);
  }, [closeRaw, cleanupSideEffects]);

  return {
    isOpen, open, close, toggle, closeTopModal, modals,
    searchMatchIds, setSearchMatchIds,
    activeSearchMatchId, setActiveSearchMatchId,
    splitViewSecondaryId, setSplitViewSecondaryId,
  };
}
