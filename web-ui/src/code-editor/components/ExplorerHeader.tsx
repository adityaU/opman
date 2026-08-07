/**
 * ExplorerHeader — the explorer's one command row.
 *
 * The header previously spent its width on a static "EXPLORER" label plus a
 * menu. The label named a panel you can already see; the width is better spent
 * on the control that actually shortens the path to a file. So the row is now
 * collapse, a live project-wide filter, and the overflow menu — three things,
 * each of which does something.
 */
import { useEffect, useRef, useState } from "react";
import {
  PanelLeftClose, MoreHorizontal, FilePlus, FolderPlus, Upload, RefreshCw,
  Pin, PinOff, Search, X, Loader2,
} from "lucide-react";

interface Props {
  pinned: boolean;
  query: string;
  searching: boolean;
  onQueryChange: (value: string) => void;
  onQueryClear: () => void;
  onTogglePinned: () => void;
  onCollapse: () => void;
  onCreateFile?: () => void;
  onCreateDir?: () => void;
  onUploadFiles?: () => void;
  onReloadRoot?: () => void;
}

interface Action { key: string; label: string; icon: React.ReactNode; run: () => void }

export function ExplorerHeader({
  pinned, query, searching, onQueryChange, onQueryClear,
  onTogglePinned, onCollapse, onCreateFile, onCreateDir, onUploadFiles, onReloadRoot,
}: Props) {
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    const dismiss = (event: Event) => {
      const target = event.target as Node;
      if (triggerRef.current?.contains(target) || menuRef.current?.contains(target)) return;
      setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setOpen(false);
      triggerRef.current?.focus();
    };
    document.addEventListener("mousedown", dismiss);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", dismiss);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const actions: Action[] = [];
  if (onCreateFile) actions.push({ key: "file", label: "New file", icon: <FilePlus size={13} />, run: onCreateFile });
  if (onCreateDir) actions.push({ key: "dir", label: "New folder", icon: <FolderPlus size={13} />, run: onCreateDir });
  if (onUploadFiles) actions.push({ key: "upload", label: "Upload files", icon: <Upload size={13} />, run: onUploadFiles });
  if (onReloadRoot) actions.push({ key: "reload", label: "Reload files", icon: <RefreshCw size={13} />, run: onReloadRoot });
  actions.push({
    key: "pin",
    label: pinned ? "Unpin explorer" : "Pin explorer open",
    icon: pinned ? <PinOff size={13} /> : <Pin size={13} />,
    run: onTogglePinned,
  });

  return (
    <div className="xpl-header">
      <button
        type="button"
        className="xpl-hdr-btn"
        onClick={onCollapse}
        title="Hide explorer"
        aria-label="Hide explorer"
      >
        <PanelLeftClose size={14} />
      </button>

      <div className={`xpl-search${query ? " has-query" : ""}`}>
        {searching
          ? <Loader2 size={12} className="xpl-search-icon spin" aria-hidden="true" />
          : <Search size={12} className="xpl-search-icon" aria-hidden="true" />}
        <input
          ref={inputRef}
          type="text"
          className="xpl-search-input"
          value={query}
          placeholder="Find a file"
          aria-label="Find a file in this project"
          spellCheck={false}
          autoComplete="off"
          onChange={(e) => onQueryChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key !== "Escape" || !query) return;
            e.preventDefault();
            e.stopPropagation();
            onQueryClear();
          }}
        />
        {query && (
          <button
            type="button"
            className="xpl-search-clear"
            aria-label="Clear filter"
            onClick={() => { onQueryClear(); inputRef.current?.focus(); }}
          >
            <X size={11} />
          </button>
        )}
      </div>

      <div className="xpl-menu-wrap">
        <button
          ref={triggerRef}
          type="button"
          className={`xpl-hdr-btn${open ? " is-active" : ""}`}
          onClick={() => setOpen((value) => !value)}
          title="Explorer actions"
          aria-label="Explorer actions"
          aria-haspopup="menu"
          aria-expanded={open}
        >
          <MoreHorizontal size={14} />
        </button>
        {open && (
          <div className="xpl-menu modal-popover-surface" role="menu" ref={menuRef}>
            {actions.map((action) => (
              <button
                key={action.key}
                type="button"
                role="menuitem"
                className="xpl-menu-item"
                onClick={() => { action.run(); setOpen(false); }}
              >
                {action.icon}
                <span>{action.label}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
