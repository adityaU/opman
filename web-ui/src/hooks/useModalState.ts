import { useState, useCallback, useRef } from "react";

export type ModalName =
  | "commandPalette" | "modelPicker" | "agentPicker" | "themeSelector"
  | "cheatsheet" | "todoPanel" | "sessionSelector" | "contextInput"
  | "settings" | "watcher" | "contextWindow" | "diffReview"
  | "searchBar" | "crossSearch" | "splitView" | "sessionGraph"
  | "sessionDashboard" | "activityFeed" | "notificationPrefs"
  | "assistantCenter" | "inbox" | "memory" | "autonomy"
  | "routines" | "delegation" | "missions" | "workspaceManager"
  | "addProject";

/** Escape-key dismiss priority, highest first. */
const ESCAPE_PRIORITY: ModalName[] = [
  "commandPalette", "modelPicker", "agentPicker", "themeSelector",
  "cheatsheet", "todoPanel", "sessionSelector", "contextInput",
  "settings", "watcher", "contextWindow", "diffReview",
  "searchBar", "crossSearch", "activityFeed", "notificationPrefs",
  "assistantCenter", "inbox", "memory", "autonomy",
  "routines", "delegation", "missions", "workspaceManager",
  "addProject",
];

type ModalOpenState = Record<ModalName, boolean>;

const INITIAL_STATE: ModalOpenState = Object.fromEntries(
  ESCAPE_PRIORITY.concat("splitView", "sessionGraph", "sessionDashboard")
    .map((k) => [k, false]),
) as ModalOpenState;

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

  const isOpen = useCallback((name: ModalName) => modalsRef.current[name], []);

  const open = useCallback((name: ModalName) => {
    setModals((prev) => (prev[name] ? prev : { ...prev, [name]: true }));
  }, []);

  const closeRaw = useCallback((name: ModalName) => {
    setModals((prev) => (prev[name] ? { ...prev, [name]: false } : prev));
  }, []);

  const close = useCallback((name: ModalName) => {
    closeRaw(name);
    if (name === "searchBar") {
      setSearchMatchIds(new Set());
      setActiveSearchMatchId(null);
    } else if (name === "splitView") {
      setSplitViewSecondaryId(null);
    }
  }, [closeRaw]);

  const toggle = useCallback((name: ModalName) => {
    setModals((prev) => {
      const next = { ...prev, [name]: !prev[name] };
      return next;
    });
    // If toggling off, run cleanup
    if (modalsRef.current[name]) {
      if (name === "searchBar") {
        setSearchMatchIds(new Set());
        setActiveSearchMatchId(null);
      } else if (name === "splitView") {
        setSplitViewSecondaryId(null);
      }
    }
  }, []);

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

  return {
    isOpen, open, close, toggle, closeTopModal, modals,
    searchMatchIds, setSearchMatchIds,
    activeSearchMatchId, setActiveSearchMatchId,
    splitViewSecondaryId, setSplitViewSecondaryId,
  };
}
