import { useRef, useEffect, useCallback, useState } from "react";
import { ptyKill } from "../api";
import { TabInfo, TabRuntime, PtyKind, KIND_LABELS, uuid } from "./types";

export function useTerminalTabs() {
  const [tabs, setTabs] = useState<TabInfo[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [renameId, setRenameId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [kindMenuOpen, setKindMenuOpen] = useState(false);

  const runtimesRef = useRef<Map<string, TabRuntime>>(new Map());
  const containerRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const tabCounter = useRef(0);

  // Close kind menu on outside click
  useEffect(() => {
    if (!kindMenuOpen) return;
    const handler = () => setKindMenuOpen(false);
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, [kindMenuOpen]);

  const createTab = useCallback((kind: PtyKind) => {
    tabCounter.current += 1;
    const id = uuid();
    const label = `${KIND_LABELS[kind]} ${tabCounter.current}`;
    const tab: TabInfo = { id, kind, label, status: "connecting" };
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
      const next = prev.filter((t) => t.id !== tabId);
      setActiveTabId((currentActive) => {
        if (currentActive !== tabId) return currentActive;
        if (next.length === 0) return null;
        const oldIdx = prev.findIndex((t) => t.id === tabId);
        const newIdx = Math.min(oldIdx, next.length - 1);
        return next[newIdx].id;
      });
      return next;
    });
  }, []);

  const closeAll = useCallback(
    (onClose: () => void) => {
      for (const [tabId, rt] of runtimesRef.current) {
        rt.observer?.disconnect();
        rt.sse?.close();
        rt.term.dispose();
        ptyKill(tabId).catch(() => {});
      }
      runtimesRef.current.clear();
      containerRefs.current.clear();
      setTabs([]);
      setActiveTabId(null);
      onClose();
    },
    []
  );

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
    closeAll,
    startRename,
    commitRename,
  };
}
