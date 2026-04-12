import React, { useState, useCallback, useMemo } from "react";
import "@xterm/xterm/css/xterm.css";
import { TerminalPanelProps } from "./types";
import { useTerminalTabs, useTerminalLifecycle, useTerminalSearch } from "./hooks";
import { TabBar, HeaderActions, SearchBar, TabBody } from "./components";

export function TerminalPanel({
  sessionId,
  onClose,
  visible = true,
  mcpAgentActive = false,
}: TerminalPanelProps) {
  const [expanded, setExpanded] = useState(false);

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
    closeAll,
    startRename,
    commitRename,
  } = useTerminalTabs();

  useTerminalLifecycle(
    tabs,
    setTabs,
    sessionId,
    runtimesRef,
    containerRefs,
    activeTabId,
    expanded,
    visible,
    createTab
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

  const handleCloseAll = useCallback(() => closeAll(onClose), [closeAll, onClose]);

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
          tabs={tabs}
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
          onCloseAll={handleCloseAll}
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
