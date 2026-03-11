import { useState, useCallback, useRef } from "react";

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

export function useMobileState(): MobileState {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [activePanel, setActivePanel] = useState<MobilePanel | null>(null);
  const [panelsMounted, setPanelsMounted] = useState<Set<string>>(new Set());
  const [inputHidden, setInputHidden] = useState(false);
  // Dock starts collapsed when input is visible (mutual exclusivity)
  const [dockCollapsed, setDockCollapsed] = useState(true);

  const hasPromptContentRef = useRef(false);
  const debounceRef = useRef(0);

  const toggleSidebar = useCallback(() => setSidebarOpen((v) => !v), []);
  const closeSidebar = useCallback(() => setSidebarOpen(false), []);

  const togglePanel = useCallback((panel: MobilePanel) => {
    setActivePanel((prev) => (prev === panel ? null : panel));
    if (panel !== "opencode") {
      setPanelsMounted((prev) => {
        if (prev.has(panel)) return prev;
        const next = new Set(prev);
        next.add(panel);
        return next;
      });
    }

    if (panel === "opencode") {
      // Chat/compose: show input, hide dock (mutual exclusivity)
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
      // Non-chat panels: show dock, hide input (mutual exclusivity)
      setDockCollapsed(false);
      setInputHidden(true);
    }
  }, []);

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
