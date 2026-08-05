/**
 * ExplorerHeader — title, collapse control, and the explorer's actions menu.
 *
 * The header used to carry five icon buttons side by side. At the explorer's
 * default width that is most of the row spent on controls that are used once a
 * session, competing with the thing the panel is actually for. They now live
 * behind one menu, and the control that was missing entirely — collapsing the
 * explorer — takes the position that reads first.
 */
import { useEffect, useRef, useState } from "react";
import {
  PanelLeftClose, MoreHorizontal, FilePlus, FolderPlus, Upload, RefreshCw, Pin, PinOff,
} from "lucide-react";

interface Props {
  pinned: boolean;
  onTogglePinned: () => void;
  onCollapse: () => void;
  onCreateFile?: () => void;
  onCreateDir?: () => void;
  onUploadFiles?: () => void;
  onReloadRoot?: () => void;
}

interface Action {
  key: string;
  label: string;
  icon: React.ReactNode;
  run: () => void;
}

export function ExplorerHeader({
  pinned, onTogglePinned, onCollapse, onCreateFile, onCreateDir, onUploadFiles, onReloadRoot,
}: Props) {
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    const dismiss = (event: Event) => {
      const target = event.target as Node;
      if (triggerRef.current?.contains(target) || menuRef.current?.contains(target)) return;
      setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
        triggerRef.current?.focus();
      }
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
    <div className="explorer-header">
      <button
        type="button"
        className="explorer-hdr-btn explorer-collapse-trigger"
        onClick={onCollapse}
        title="Hide explorer"
        aria-label="Hide explorer"
      >
        <PanelLeftClose size={14} />
      </button>
      <span className="explorer-title">Explorer</span>
      <div className="explorer-menu-wrap">
        <button
          ref={triggerRef}
          type="button"
          className={`explorer-hdr-btn${open ? " is-active" : ""}`}
          onClick={() => setOpen((value) => !value)}
          title="Explorer actions"
          aria-label="Explorer actions"
          aria-haspopup="menu"
          aria-expanded={open}
        >
          <MoreHorizontal size={14} />
        </button>
        {open && (
          <div className="explorer-menu modal-popover-surface" role="menu" ref={menuRef}>
            {actions.map((action) => (
              <button
                key={action.key}
                type="button"
                role="menuitem"
                className="explorer-menu-item"
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
