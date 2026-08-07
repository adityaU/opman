import { useState, useCallback, useRef, useEffect } from "react";
import { MODAL_HISTORY_KEY } from "../utils/navigation";

export type ModalName =
  | "commandPalette" | "modelPicker" | "agentPicker"
  | "todoPanel" | "sessionSelector" | "contextInput"
  | "watcher" | "contextWindow" | "diffReview"
  | "searchBar" | "crossSearch" | "notificationPrefs"
  | "memory" | "autonomy" | "routines"
  | "addProject" | "systemMonitor" | "processHealth" | "autoOpen";

/** Escape-key dismiss priority, highest first. */
const ESCAPE_PRIORITY: ModalName[] = [
  "commandPalette", "modelPicker", "agentPicker",
  "todoPanel", "sessionSelector", "contextInput",
  "watcher", "contextWindow", "diffReview",
  "searchBar", "crossSearch", "notificationPrefs",
  "memory", "autonomy", "routines",
  "addProject", "systemMonitor", "processHealth", "autoOpen",
];

type ModalOpenState = Record<ModalName, boolean>;

const INITIAL_STATE: ModalOpenState = Object.fromEntries(
  ESCAPE_PRIORITY.map((k) => [k, false]),
) as ModalOpenState;


/**
 * Whether the current history entry is one a modal open pushed.
 *
 * `appNavigate` pushes with a null state, so a page navigation is distinguishable from a
 * modal entry — which is what tells a closing modal whether `history.back()` would unwind
 * itself or somebody else's navigation.
 */
function ownsTopHistoryEntry(): boolean {
  const state = window.history.state as Record<string, unknown> | null;
  return Boolean(state && MODAL_HISTORY_KEY in state);
}

export interface ModalStateAPI {
  /** Check whether a modal is currently open. */
  isOpen: (name: ModalName) => boolean;
  /** Open a modal by name. */
  open: (name: ModalName) => void;
  /** Close a modal by name (with side-effect cleanup for searchBar/memory). */
  close: (name: ModalName) => void;
  /** Close a modal without navigating browser history. */
  closeSilent: (name: ModalName) => void;
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
  /** Open memory modal showing only active (scoped) memories. */
  openMemoryActive: () => void;
  /** Open memory modal showing all memories. */
  openMemoryAll: () => void;
  /** Whether the memory modal should filter to active-only items. */
  memoryFilterActive: boolean;
}

export interface ModalStateOptions {
  /** Called whenever any modal is opened. Use to block SSE session adoption. */
  onOpen?: () => void;
}

export function useModalState(options?: ModalStateOptions): ModalStateAPI {
  const [modals, setModals] = useState<ModalOpenState>(INITIAL_STATE);
  const [searchMatchIds, setSearchMatchIds] = useState<Set<string>>(new Set());
  const [activeSearchMatchId, setActiveSearchMatchId] = useState<string | null>(null);
  const [memoryFilterActive, setMemoryFilterActive] = useState(false);

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
    } else if (name === "memory") {
      setMemoryFilterActive(false);
    }
  }, []);

  // ── Public API ──────────────────────────────────────────────────

  const open = useCallback((name: ModalName) => {
    setModals((prev) => {
      if (prev[name]) return prev; // already open
      // Block any SSE-driven session adoption while a modal is open
      options?.onOpen?.();
      // Push a history entry so the back gesture closes this modal
      window.history.pushState({ [MODAL_HISTORY_KEY]: true }, "");
      historyDepthRef.current += 1;
      return { ...prev, [name]: true };
    });
  }, [options]);

  const close = useCallback((name: ModalName) => {
    setModals((prev) => {
      if (!prev[name]) return prev; // already closed
      // Pop the matching history entry we pushed on open
      if (historyDepthRef.current > 0) {
        historyDepthRef.current -= 1;
        // Only unwind while the entry on top is still the one this modal pushed.
        // A navigation since then sits above it — a command palette row that opens
        // a *page* is the case that broke: the row pushed `/settings`, this close
        // ran `history.back()`, and the browser popped the page rather than the
        // modal, landing the user back in the session they came from.
        if (ownsTopHistoryEntry()) {
          suppressPopRef.current = true;
          window.history.back();
        }
      }
      return { ...prev, [name]: false };
    });
    cleanupSideEffects(name);
  }, [cleanupSideEffects]);

  const closeSilent = useCallback((name: ModalName) => {
    setModals((prev) => {
      if (!prev[name]) return prev;
      if (historyDepthRef.current > 0) {
        historyDepthRef.current -= 1;
        if (ownsTopHistoryEntry()) {
          const st = window.history.state as Record<string, unknown>;
          const { [MODAL_HISTORY_KEY]: _, ...rest } = st;
          window.history.replaceState(Object.keys(rest).length ? rest : null, "");
        }
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
      // Nothing else pushes entries any more: session and panel state left the
      // URL, so the only other producers are the page routes themselves.
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

  const openMemoryActive = useCallback(() => {
    setMemoryFilterActive(true);
    open("memory");
  }, [open]);

  const openMemoryAll = useCallback(() => {
    setMemoryFilterActive(false);
    open("memory");
  }, [open]);

  return {
    isOpen, open, close, closeSilent, toggle, closeTopModal, modals,
    searchMatchIds, setSearchMatchIds,
    activeSearchMatchId, setActiveSearchMatchId,
    openMemoryActive, openMemoryAll, memoryFilterActive,
  };
}
