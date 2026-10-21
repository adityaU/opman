import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import "@xterm/xterm/css/xterm.css";
import type { TerminalPanelProps } from "./types";
import { useActiveShell } from "./useActiveShell";
import { useTerminalSession } from "./useTerminalSession";
import { useTerminalSearch } from "./useTerminalSearch";
import { SearchBar } from "./components";
import { ShellPicker } from "./ShellPicker";
import { MobileKeyBar } from "./mobile/MobileKeyBar";
import { useTerminalCommands } from "./useTerminalCommands";
import { useMobileKeys } from "./mobile/useMobileKeys";
import { TerminalMenu } from "./TerminalMenu";
import type { FeatureHooks, TerminalActions } from "./features";

const NOTICE_MS = 2200;

/**
 * One terminal showing one shell, and nothing else on screen.
 *
 * There is deliberately no header: every row of chrome is a row of scrollback,
 * and everything the header offered lives elsewhere — switching, renaming and
 * killing shells in the picker (`terminal.selectShell`), search on Ctrl+F,
 * expand on its command. The pane's own title bar already says this is a
 * terminal.
 *
 * The shell belongs to the project, not to this panel: it was very likely
 * started by another pane, and it will outlive this one.
 */
export function TerminalPanel({
  sessionId,
  projectPath,
  visible = true,
  layout = "desktop",
  ptyId = null,
  onPtyIdChanged,
}: TerminalPanelProps) {
  const isMobile = layout === "mobile";
  const [expanded, setExpanded] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const active = useActiveShell(ptyId, projectPath, sessionId, onPtyIdChanged);

  // The touch key bar rewrites keystrokes (sticky Ctrl/Alt), so the terminal's
  // data path reads the transform from this ref.
  const transformRef = useRef<((data: string) => string) | null>(null);

  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [copyMode, setCopyMode] = useState(false);
  const hooks = useMemo<FeatureHooks>(
    () => ({
      onMenu: (x, y) => setMenu({ x, y }),
      onNotice: setNotice,
      onCopyMode: setCopyMode,
    }),
    [],
  );
  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(null), NOTICE_MS);
    return () => clearTimeout(timer);
  }, [notice]);

  const { status, runtimeRef } = useTerminalSession(
    active.ptyId,
    active.shell?.kind ?? "shell",
    projectPath,
    sessionId,
    containerRef,
    visible && !active.choosing,
    transformRef,
    hooks,
  );
  const act = useCallback(
    <T,>(run: (actions: TerminalActions) => T) => {
      const actions = runtimeRef.current?.actions;
      return actions ? run(actions) : undefined;
    },
    [runtimeRef],
  );

  const mobileKeys = useMobileKeys(active.ptyId, runtimeRef, transformRef, isMobile);
  const search = useTerminalSearch(runtimeRef);

  useTerminalCommands({
    hasShell: active.ptyId !== null,
    newShell: active.create,
    killShell: active.ptyId ? () => active.kill(active.ptyId as string) : undefined,
    selectShell: active.startChoosing,
    step: active.step,
    clear: () => runtimeRef.current?.term.clear(),
    expand: () => setExpanded((on) => !on),
    find: search.toggleSearch,
    copy: () => act((a) => void a.copy()),
    paste: () => act((a) => void a.paste()),
    selectAll: () => act((a) => a.selectAll()),
    copyMode: () => act((a) => a.copyMode()),
    leaveFocus: () => act((a) => a.leaveFocus()),
    zoom: (step) => act((a) => a.zoom(step)),
    scrollTop: () => act((a) => a.scrollTop()),
    scrollBottom: () => act((a) => a.scrollBottom()),
  });

  return (
    <div
      className={`terminal-panel ${expanded ? "expanded" : ""}${isMobile ? " terminal-panel-mobile" : ""}`}
      data-surface="terminal"
    >
      {search.searchOpen && !active.choosing && (
        <SearchBar
          searchQuery={search.searchQuery}
          options={search.options}
          results={search.results}
          onToggle={search.toggle}
          searchInputRef={search.searchInputRef}
          onSearchChange={search.handleSearchChange}
          onSearchNext={search.searchNext}
          onSearchPrev={search.searchPrev}
          onClose={search.closeSearch}
        />
      )}

      <div className="terminal-panel-body">
        {/* The terminal stays mounted behind the picker so switching back to it
            does not tear down and re-attach the stream. */}
        <div
          ref={containerRef}
          className="term-surface"
          style={{ display: active.choosing ? "none" : "block" }}
        >
          {copyMode && (
            <div className="terminal-badge" role="status">
              Copy mode — j/k move, v select, y copy, q quit
            </div>
          )}
          {notice && (
            <div className="terminal-badge is-notice" role="status">
              {notice}
            </div>
          )}
          {status === "connecting" && <div className="terminal-overlay">Opening terminal…</div>}
          {status === "error" && (
            <div className="terminal-overlay error">This terminal could not be opened</div>
          )}
        </div>

        {active.choosing && (
          <ShellPicker
            shells={active.shells}
            loading={active.loading}
            projectName={basename(projectPath) ?? "this project"}
            onPick={active.select}
            onCreate={active.create}
            onKill={active.kill}
            onRename={active.renameById}
            onCancel={active.ptyId ? active.stopChoosing : undefined}
          />
        )}
      </div>

      {menu && (
        <TerminalMenu
          x={menu.x}
          y={menu.y}
          canCopy={runtimeRef.current?.actions?.hasSelection() ?? false}
          onCopy={() => act((a) => void a.copy())}
          onPaste={() => act((a) => void a.paste())}
          onSelectAll={() => act((a) => a.selectAll())}
          onFind={search.toggleSearch}
          onCopyMode={() => act((a) => a.copyMode())}
          onClear={() => runtimeRef.current?.term.clear()}
          onClose={() => {
            setMenu(null);
            runtimeRef.current?.term.focus();
          }}
        />
      )}

      {isMobile && !active.choosing && <MobileKeyBar keys={mobileKeys} />}
    </div>
  );
}

function basename(path: string | null): string | null {
  if (!path) return null;
  const parts = path.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}
