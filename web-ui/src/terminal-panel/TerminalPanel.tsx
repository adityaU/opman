import React, { useState, useCallback, useMemo, useEffect, useRef } from "react";
import "@xterm/xterm/css/xterm.css";
import { TerminalPanelProps } from "./types";
import { useTerminalTabs, useTerminalLifecycle, useTerminalSearch } from "./hooks";
import { TabBar, HeaderActions, SearchBar, TabBody } from "./components";
import { MobileKeyBar } from "./mobile/MobileKeyBar";
import { useTerminalCommands } from "./useTerminalCommands";
import { useMobileKeys } from "./mobile/useMobileKeys";

export function TerminalPanel({
  sessionId,
  projectPath,
  onClose,
  visible = true,
  mcpAgentActive = false,
  layout = "desktop",
  restoreIds,
  onTabsChanged,
}: TerminalPanelProps) {
  const isMobile = layout === "mobile";
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
  } = useTerminalTabs(projectKey, restoreIds, onTabsChanged);

  // The touch key bar rewrites keystrokes (sticky Ctrl/Alt), so the terminal's
  // data path reads the transform from this ref.
  const transformRef = useRef<((data: string) => string) | null>(null);

  useTerminalLifecycle(
    tabs,
    setTabs,
    sessionId,
    runtimesRef,
    containerRefs,
    activeTabId,
    expanded,
    visible,
    transformRef
  );

  const mobileKeys = useMobileKeys(activeTabId, runtimesRef, transformRef, isMobile);

  useTerminalCommands({
    tabs, activeTabId, setActiveTabId, createTab, closeTab, startRename,
    openKindMenu: () => setKindMenuOpen(true),
    expand: () => setExpanded((on) => !on),
  });

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
    <div className={`terminal-panel ${expanded ? "expanded" : ""}${isMobile ? " terminal-panel-mobile" : ""}`} data-surface="terminal">
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

      {isMobile && <MobileKeyBar keys={mobileKeys} />}
    </div>
  );
}
