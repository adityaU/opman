import React from "react";
import {
  X,
  Maximize2,
  Minimize2,
  Plus,
  Terminal as TermIcon,
  Search,
  ChevronUp,
  ChevronDown,
} from "lucide-react";
import { TabInfo, PtyKind, KIND_LABELS, ALL_PTY_KINDS } from "./types";

// ── Tab Bar ────────────────────────────────────────────

interface TabBarProps {
  tabs: TabInfo[];
  activeTabId: string | null;
  renameId: string | null;
  renameValue: string;
  kindMenuOpen: boolean;
  onSelectTab: (id: string) => void;
  onStartRename: (id: string) => void;
  onRenameValueChange: (value: string) => void;
  onCommitRename: () => void;
  onCancelRename: () => void;
  onCloseTab: (id: string) => void;
  onToggleKindMenu: () => void;
  onCreateTab: (kind: PtyKind) => void;
}

export function TabBar({
  tabs,
  activeTabId,
  renameId,
  renameValue,
  kindMenuOpen,
  onSelectTab,
  onStartRename,
  onRenameValueChange,
  onCommitRename,
  onCancelRename,
  onCloseTab,
  onToggleKindMenu,
  onCreateTab,
}: TabBarProps) {
  return (
    <div className="term-tab-bar">
      {tabs.map((tab) => (
        <div
          key={tab.id}
          className={`term-tab ${tab.id === activeTabId ? "active" : ""}`}
          onClick={() => onSelectTab(tab.id)}
          onDoubleClick={() => onStartRename(tab.id)}
          title={`${KIND_LABELS[tab.kind]} — ${tab.label}`}
        >
          <TermIcon size={11} className="term-tab-icon" />
          {renameId === tab.id ? (
            <input
              className="term-tab-rename"
              value={renameValue}
              onChange={(e) => onRenameValueChange(e.target.value)}
              onBlur={onCommitRename}
              onKeyDown={(e) => {
                if (e.key === "Enter") onCommitRename();
                if (e.key === "Escape") onCancelRename();
              }}
              autoFocus
              onClick={(e) => e.stopPropagation()}
            />
          ) : (
            <span className="term-tab-label">{tab.label}</span>
          )}
          {tabs.length > 1 && (
            <button
              className="term-tab-close"
              onClick={(e) => {
                e.stopPropagation();
                onCloseTab(tab.id);
              }}
              title="Close tab"
            >
              <X size={10} />
            </button>
          )}
        </div>
      ))}

      <div className="term-tab-new-wrapper">
        <button
          className="term-tab-new"
          onClick={() => onToggleKindMenu()}
          title="New terminal tab"
        >
          <Plus size={12} />
        </button>
        {kindMenuOpen && (
          <div className="term-kind-menu">
            {ALL_PTY_KINDS.map((k) => (
              <button
                key={k}
                className="term-kind-item"
                onClick={() => onCreateTab(k)}
              >
                {KIND_LABELS[k]}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ── Header Actions ─────────────────────────────────────

interface HeaderActionsProps {
  expanded: boolean;
  searchOpen: boolean;
  mcpAgentActive: boolean;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  onToggleSearch: () => void;
  onToggleExpand: () => void;
  onCloseAll: () => void;
}

export function HeaderActions({
  expanded,
  searchOpen,
  mcpAgentActive,
  searchInputRef,
  onToggleSearch,
  onToggleExpand,
  onCloseAll,
}: HeaderActionsProps) {
  return (
    <div className="terminal-panel-actions">
      {mcpAgentActive && (
        <span className="mcp-agent-indicator" title="AI agent active">
          <span className="mcp-agent-dot" />
        </span>
      )}
      <button
        onClick={onToggleSearch}
        title="Find (Cmd+F)"
        aria-label="Search in terminal"
        className={searchOpen ? "active" : ""}
      >
        <Search size={14} />
      </button>
      <button
        onClick={onToggleExpand}
        title="Toggle size"
        aria-label={expanded ? "Minimize terminal" : "Maximize terminal"}
      >
        {expanded ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
      </button>
      <button
        onClick={onCloseAll}
        title="Close terminal panel"
        aria-label="Close terminal panel"
      >
        <X size={14} />
      </button>
    </div>
  );
}

// ── Search Bar ─────────────────────────────────────────

interface SearchBarProps {
  searchQuery: string;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  onSearchChange: (query: string) => void;
  onSearchNext: () => void;
  onSearchPrev: () => void;
  onClose: () => void;
}

export function SearchBar({
  searchQuery,
  searchInputRef,
  onSearchChange,
  onSearchNext,
  onSearchPrev,
  onClose,
}: SearchBarProps) {
  return (
    <div className="term-search-bar">
      <Search size={12} className="term-search-icon" />
      <input
        ref={searchInputRef as React.RefObject<HTMLInputElement>}
        className="term-search-input"
        type="text"
        placeholder="Find in terminal..."
        value={searchQuery}
        onChange={(e) => onSearchChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            if (e.shiftKey) onSearchPrev();
            else onSearchNext();
          }
          if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          }
        }}
      />
      <button className="term-search-nav" onClick={onSearchPrev} title="Previous match (Shift+Enter)">
        <ChevronUp size={14} />
      </button>
      <button className="term-search-nav" onClick={onSearchNext} title="Next match (Enter)">
        <ChevronDown size={14} />
      </button>
      <button className="term-search-close" onClick={onClose} title="Close search">
        <X size={12} />
      </button>
    </div>
  );
}

// ── Tab Body ───────────────────────────────────────────

interface TabBodyProps {
  tabs: TabInfo[];
  activeTabId: string | null;
  containerRefs: React.MutableRefObject<Map<string, HTMLDivElement>>;
}

export function TabBody({ tabs, activeTabId, containerRefs }: TabBodyProps) {
  return (
    <div className="terminal-panel-body">
      {tabs.map((tab) => (
        <div
          key={tab.id}
          ref={(el) => {
            if (el) containerRefs.current.set(tab.id, el);
          }}
          className="term-tab-body"
          style={{ display: tab.id === activeTabId ? "block" : "none" }}
        >
          {tab.status === "connecting" && (
            <div className="terminal-overlay">
              Spawning {KIND_LABELS[tab.kind]}...
            </div>
          )}
          {tab.status === "error" && (
            <div className="terminal-overlay error">
              Failed to start {KIND_LABELS[tab.kind]}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
