import React, { useState, useCallback, useMemo, useEffect, useRef } from "react";
import "@xterm/xterm/css/xterm.css";
import { TerminalPanelProps } from "./types";
import { useTerminalTabs, useTerminalLifecycle, useTerminalSearch } from "./hooks";
import { TabBar, HeaderActions, SearchBar, TabBody } from "./components";

export function TerminalPanel({
  sessionId,
  projectPath,
  onClose,
  visible = true,
  mcpAgentActive = false,
  attachNonce,
  attachKind = "claude-attach",
}: TerminalPanelProps) {
  const [expanded, setExpanded] = useState(false);
  const projectKey = projectPath ?? "default";

  const {
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
  } = useTerminalTabs(projectKey);

  useTerminalLifecycle(
    tabs,
    setTabs,
    sessionId,
    runtimesRef,
    containerRefs,
    activeTabId,
    expanded,
    visible
  );

  // Only the active project's tabs are shown in the tab bar — tabs for other
  // projects stay mounted in TabBody so their terminals keep running.
  const visibleTabs = useMemo(
    () => tabs.filter((t) => t.projectKey === projectKey),
    [tabs, projectKey]
  );

  const {
    searchOpen,
    setSearchOpen,
    searchQuery,
    searchInputRef,
    handleSearchChange,
    searchNext,
    searchPrev,
    closeSearch,
  } = useTerminalSearch(activeTabId, runtimesRef);

  // Open a fresh attach tab whenever the caller bumps `attachNonce` (the input's
  // "Attach terminal" button). Skip the initial render so it doesn't fire spuriously.
  const lastAttachNonce = useRef<number | undefined>(attachNonce);
  useEffect(() => {
    if (attachNonce === undefined || attachNonce === lastAttachNonce.current) return;
    lastAttachNonce.current = attachNonce;
    createTab(attachKind);
  }, [attachNonce, attachKind, createTab]);

  const handleHidePanel = useCallback(() => onClose(), [onClose]);

  const handleToggleSearch = useCallback(() => {
    if (searchOpen) {
      closeSearch();
    } else {
      setSearchOpen(true);
      requestAnimationFrame(() => searchInputRef.current?.focus());
    }
  }, [searchOpen, closeSearch, setSearchOpen, searchInputRef]);

  const handleCancelRename = useCallback(() => {
    setRenameId(null);
    setRenameValue("");
  }, [setRenameId, setRenameValue]);

  const activeTab = useMemo(
    () => tabs.find((t) => t.id === activeTabId) ?? null,
    [tabs, activeTabId]
  );

  return (
    <div className={`terminal-panel ${expanded ? "expanded" : ""}`}>
      <div className="terminal-panel-header">
        <TabBar
          tabs={visibleTabs}
          activeTabId={activeTabId}
          renameId={renameId}
          renameValue={renameValue}
          kindMenuOpen={kindMenuOpen}
          onSelectTab={setActiveTabId}
          onStartRename={startRename}
          onRenameValueChange={setRenameValue}
          onCommitRename={commitRename}
          onCancelRename={handleCancelRename}
          onCloseTab={closeTab}
          onToggleKindMenu={() => setKindMenuOpen((v) => !v)}
          onCreateTab={createTab}
        />
        <HeaderActions
          expanded={expanded}
          searchOpen={searchOpen}
          mcpAgentActive={mcpAgentActive}
          searchInputRef={searchInputRef}
          onToggleSearch={handleToggleSearch}
          onToggleExpand={() => setExpanded((v) => !v)}
          onHidePanel={handleHidePanel}
        />
      </div>

      {searchOpen && (
        <SearchBar
          searchQuery={searchQuery}
          searchInputRef={searchInputRef}
          onSearchChange={handleSearchChange}
          onSearchNext={searchNext}
          onSearchPrev={searchPrev}
          onClose={closeSearch}
        />
      )}

      <TabBody
        tabs={tabs}
        activeTabId={activeTabId}
        containerRefs={containerRefs}
      />
    </div>
  );
}
