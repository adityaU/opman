import { useRef, useEffect, useCallback, useState } from "react";
import { ptyKill } from "../api";
import { TabInfo, TabRuntime, PtyKind, KIND_LABELS, uuid } from "./types";

export function useTerminalTabs(projectKey: string) {
  const [tabs, setTabs] = useState<TabInfo[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [renameId, setRenameId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [kindMenuOpen, setKindMenuOpen] = useState(false);

  const runtimesRef = useRef<Map<string, TabRuntime>>(new Map());
  const containerRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const tabCounter = useRef(0);
  const projectKeyRef = useRef(projectKey);
  projectKeyRef.current = projectKey;
  const activeTabByProjectRef = useRef<Map<string, string>>(new Map());

  // Remember which tab was active for the current project, so switching
  // projects and back restores the same tab instead of picking the first one.
  useEffect(() => {
    if (activeTabId) activeTabByProjectRef.current.set(projectKey, activeTabId);
  }, [projectKey, activeTabId]);

  // When the active project changes, switch to its remembered/first tab,
  // or spin up a fresh shell if it has none yet. Tabs for other projects
  // stay mounted (see TabBody) so their terminals keep running untouched.
  useEffect(() => {
    setTabs((currentTabs) => {
      const forProject = currentTabs.filter((t) => t.projectKey === projectKey);
      if (forProject.length === 0) {
        tabCounter.current += 1;
        const id = uuid();
        const label = `${KIND_LABELS.shell} ${tabCounter.current}`;
        const tab: TabInfo = { id, kind: "shell", label, status: "connecting", projectKey };
        setActiveTabId(id);
        return [...currentTabs, tab];
      }
      const remembered = activeTabByProjectRef.current.get(projectKey);
      const restored = forProject.find((t) => t.id === remembered) ?? forProject[0];
      setActiveTabId(restored.id);
      return currentTabs;
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectKey]);

  // Close kind menu on outside click (use mousedown to avoid race with React onClick)
  useEffect(() => {
    if (!kindMenuOpen) return;
    const handler = (e: MouseEvent) => {
      const wrapper = document.querySelector(".term-tab-new-wrapper");
      if (wrapper && wrapper.contains(e.target as Node)) return;
      setKindMenuOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [kindMenuOpen]);

  const createTab = useCallback((kind: PtyKind) => {
    tabCounter.current += 1;
    const id = uuid();
    const label = `${KIND_LABELS[kind]} ${tabCounter.current}`;
    const tab: TabInfo = { id, kind, label, status: "connecting", projectKey: projectKeyRef.current };
    setTabs((prev) => [...prev, tab]);
    setActiveTabId(id);
    setKindMenuOpen(false);
  }, []);

  const closeTab = useCallback((tabId: string) => {
    const rt = runtimesRef.current.get(tabId);
    if (rt) {
      rt.observer?.disconnect();
      rt.sse?.close();
      rt.term.dispose();
      runtimesRef.current.delete(tabId);
    }
    ptyKill(tabId).catch(() => {});
    containerRefs.current.delete(tabId);

    setTabs((prev) => {
      const closed = prev.find((t) => t.id === tabId);
      const next = prev.filter((t) => t.id !== tabId);
      setActiveTabId((currentActive) => {
        if (currentActive !== tabId || !closed) return currentActive;
        const siblings = prev.filter((t) => t.projectKey === closed.projectKey && t.id !== tabId);
        if (siblings.length === 0) return null;
        const oldIdx = prev.filter((t) => t.projectKey === closed.projectKey).findIndex((t) => t.id === tabId);
        const newIdx = Math.min(oldIdx, siblings.length - 1);
        return siblings[newIdx].id;
      });
      return next;
    });
  }, []);

  const startRename = useCallback(
    (tabId: string) => {
      const tab = tabs.find((t) => t.id === tabId);
      if (tab) {
        setRenameId(tabId);
        setRenameValue(tab.label);
      }
    },
    [tabs]
  );

  const commitRename = useCallback(() => {
    if (renameId && renameValue.trim()) {
      setTabs((prev) =>
        prev.map((t) =>
          t.id === renameId ? { ...t, label: renameValue.trim() } : t
        )
      );
    }
    setRenameId(null);
    setRenameValue("");
  }, [renameId, renameValue]);

  return {
    tabs,
    setTabs,
    activeTabId,
    setActiveTabId,
    renameId,
    setRenameId,
    renameValue,
    setRenameValue,
    kindMenuOpen,
    setKindMenuOpen,
    runtimesRef,
    containerRefs,
    createTab,
    closeTab,
    startRename,
    commitRename,
  };
}
