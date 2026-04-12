import { useState, useCallback } from "react";
import type { GitView } from "../types";

/**
 * Manages the breadcrumb view-stack navigation for the git panel.
 */
export function useViewNavigation() {
  const [viewStack, setViewStack] = useState<GitView[]>([{ kind: "list" }]);
  const [breadcrumbDropdown, setBreadcrumbDropdown] = useState(false);

  const currentView = viewStack[viewStack.length - 1];

  const pushView = useCallback((view: GitView) => {
    setViewStack((s) => [...s, view]);
    setBreadcrumbDropdown(false);
  }, []);

  const popView = useCallback(() => {
    setViewStack((s) => (s.length > 1 ? s.slice(0, -1) : s));
    setBreadcrumbDropdown(false);
  }, []);

  const jumpToView = useCallback((index: number) => {
    setViewStack((s) => s.slice(0, index + 1));
    setBreadcrumbDropdown(false);
  }, []);

  const resetStack = useCallback(() => {
    setViewStack([{ kind: "list" }]);
    setBreadcrumbDropdown(false);
  }, []);

  return {
    viewStack,
    currentView,
    breadcrumbDropdown,
    setBreadcrumbDropdown,
    pushView,
    popView,
    jumpToView,
    resetStack,
  };
}
