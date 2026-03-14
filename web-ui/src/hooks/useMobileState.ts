import { useState, useCallback, useRef, useEffect } from "react";

type MobilePanel = "opencode" | "git" | "editor" | "terminal";

export interface MobileState {
  sidebarOpen: boolean;
  setSidebarOpen: (v: boolean) => void;
  toggleSidebar: () => void;
  closeSidebar: () => void;
  activePanel: MobilePanel | null;
  panelsMounted: Set<string>;
  togglePanel: (panel: MobilePanel) => void;
  inputHidden: boolean;
  setInputHidden: (v: boolean) => void;
  dockCollapsed: boolean;
  expandDock: () => void;
  handleScrollDirection: (direction: "up" | "down") => void;
  handlePromptContentChange: (hasContent: boolean) => void;
  handleComposeButtonTap: () => void;
}

/** Sentinel on history.state to distinguish mobile-overlay entries. */
const MOBILE_HISTORY_KEY = "_mobileOverlay";

export function useMobileState(): MobileState {
  const [sidebarOpen, setSidebarOpenRaw] = useState(false);
  const [activePanel, setActivePanelRaw] = useState<MobilePanel | null>(null);
  const [panelsMounted, setPanelsMounted] = useState<Set<string>>(new Set());
  const [inputHidden, setInputHidden] = useState(false);
  // Dock starts collapsed when input is visible (mutual exclusivity)
  const [dockCollapsed, setDockCollapsed] = useState(true);

  const hasPromptContentRef = useRef(false);
  const debounceRef = useRef(0);

  // ── Back-gesture history refs ────────────────────────────────────
  /** How many history entries we have pushed for mobile overlays. */
  const historyDepthRef = useRef(0);
  /** Guard: true while we are programmatically navigating history. */
  const suppressPopRef = useRef(false);

  // Keep refs so the popstate handler always sees current values.
  const sidebarOpenRef = useRef(sidebarOpen);
  sidebarOpenRef.current = sidebarOpen;
  const activePanelRef = useRef(activePanel);
  activePanelRef.current = activePanel;

  // ── History helpers ──────────────────────────────────────────────

  const pushOverlayHistory = useCallback(() => {
    window.history.pushState({ [MOBILE_HISTORY_KEY]: true }, "");
    historyDepthRef.current += 1;
  }, []);

  const popOverlayHistory = useCallback(() => {
    if (historyDepthRef.current > 0) {
      historyDepthRef.current -= 1;
      suppressPopRef.current = true;
      window.history.back();
    }
  }, []);

  // ── Sidebar ──────────────────────────────────────────────────────

  const setSidebarOpen = useCallback((v: boolean) => {
    setSidebarOpenRaw((prev) => {
      if (v === prev) return prev;
      if (v) pushOverlayHistory();
      else popOverlayHistory();
      return v;
    });
  }, [pushOverlayHistory, popOverlayHistory]);

  const toggleSidebar = useCallback(() => {
    setSidebarOpenRaw((prev) => {
      const next = !prev;
      if (next) pushOverlayHistory();
      else popOverlayHistory();
      return next;
    });
  }, [pushOverlayHistory, popOverlayHistory]);

  const closeSidebar = useCallback(() => {
    setSidebarOpenRaw((prev) => {
      if (!prev) return prev;
      popOverlayHistory();
      return false;
    });
  }, [popOverlayHistory]);

  // ── Panels ───────────────────────────────────────────────────────

  const togglePanel = useCallback((panel: MobilePanel) => {
    setActivePanelRaw((prev) => {
      const closing = prev === panel;
      if (closing) {
        popOverlayHistory();
        // Closing a panel — restore input/dock defaults handled below
      } else {
        // Opening a new panel (possibly replacing an existing one)
        if (prev === null) {
          // No panel was open — push history
          pushOverlayHistory();
        }
        // If replacing one panel with another, history depth stays the same
      }
      return closing ? null : panel;
    });

    if (panel !== "opencode") {
      setPanelsMounted((prev) => {
        if (prev.has(panel)) return prev;
        const next = new Set(prev);
        next.add(panel);
        return next;
      });
    }

    // The rest of the input/dock logic runs unconditionally (same as before)
    if (panel === "opencode") {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
        debounceRef.current = 0;
      }
      setInputHidden(false);
      setDockCollapsed(true);
      requestAnimationFrame(() => {
        const textarea = document.querySelector<HTMLTextAreaElement>(".prompt-textarea");
        textarea?.focus();
      });
    } else {
      setDockCollapsed(false);
      setInputHidden(true);
    }
  }, [pushOverlayHistory, popOverlayHistory]);

  const expandDock = useCallback(() => {
    // Show dock, hide input (mutual exclusivity)
    setDockCollapsed(false);
    setInputHidden(true);
  }, []);

  const handleScrollDirection = useCallback((direction: "up" | "down") => {
    if (typeof window !== "undefined" && window.innerWidth >= 768) return;

    if (direction === "down") {
      // Scrolling down — do nothing.
      // Dock only reappears when the user taps the menu FAB.
      return;
    }

    // Scrolling up (toward older messages) — collapse dock + hide input
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(() => {
      if (!hasPromptContentRef.current) {
        setInputHidden(true);
        setDockCollapsed(true);
      }
      debounceRef.current = 0;
    }, 150);
  }, []);

  const handlePromptContentChange = useCallback((hasContent: boolean) => {
    hasPromptContentRef.current = hasContent;
  }, []);

  const handleComposeButtonTap = useCallback(() => {
    // Cancel any pending scroll-hide debounce so it can't re-hide after reveal
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
      debounceRef.current = 0;
    }
    // Show input, hide dock (mutual exclusivity)
    setInputHidden(false);
    setDockCollapsed(true);
    requestAnimationFrame(() => {
      const textarea = document.querySelector<HTMLTextAreaElement>(".prompt-textarea");
      textarea?.focus();
    });
  }, []);

  // ── Back-gesture / popstate listener ────────────────────────────
  useEffect(() => {
    const handler = (e: PopStateEvent) => {
      // If we triggered this popstate ourselves (via history.back() in close/toggle),
      // just consume the event — the overlay is already closed.
      if (suppressPopRef.current) {
        suppressPopRef.current = false;
        return;
      }

      // Only handle our own history entries
      if (historyDepthRef.current <= 0) return;

      historyDepthRef.current -= 1;

      // Close the top-most mobile overlay: panel first, then sidebar
      if (activePanelRef.current !== null) {
        setActivePanelRaw(null);
        // Restore input/dock defaults when panel closes via back
        setInputHidden(false);
        setDockCollapsed(true);
      } else if (sidebarOpenRef.current) {
        setSidebarOpenRaw(false);
      }
    };
    window.addEventListener("popstate", handler);
    return () => window.removeEventListener("popstate", handler);
  }, []);

  return {
    sidebarOpen,
    setSidebarOpen,
    toggleSidebar,
    closeSidebar,
    activePanel,
    panelsMounted,
    togglePanel,
    inputHidden,
    setInputHidden,
    dockCollapsed,
    expandDock,
    handleScrollDirection,
    handlePromptContentChange,
    handleComposeButtonTap,
  };
}
