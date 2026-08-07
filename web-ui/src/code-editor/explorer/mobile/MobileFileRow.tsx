/**
 * MobileFileRow — one entry in the mobile browser.
 *
 * The row carries the same typed tile as the desktop tree, so a folder full of
 * files reads by colour and shape rather than word by word, and it is sized for
 * a thumb rather than a cursor. Actions stay behind a swipe: a phone row has no
 * hover, and putting three buttons on every row would leave no width for the
 * name they act on.
 */
import { ChevronRight, Folder, Trash2, Pencil, Download } from "lucide-react";
import type { FileEntry } from "../../types";
import { formatSize } from "../../types";
import { useSwipeReveal } from "../../../hooks/useSwipeReveal";
import { FileTile, MatchedName } from "../ExplorerBits";

/** Width of one revealed action button — matches `.xplm-swipe-btn` in CSS. */
const ACTION_WIDTH = 56;

interface Props {
  entry: FileEntry;
  /** Highlighted when the row came from a search. */
  query?: string;
  /** Shown under the name for search results, where the folder is not implied. */
  subtitle?: string;
  onOpen: (entry: FileEntry) => void;
  onRename?: () => void;
  onDownload?: () => void;
  onDelete?: () => void;
}

export function MobileFileRow({
  entry, query, subtitle, onOpen, onRename, onDownload, onDelete,
}: Props) {
  const actions = [
    onRename && { key: "rename", label: "Rename", icon: <Pencil size={16} />, tone: "primary", run: onRename },
    onDownload && { key: "download", label: "Download", icon: <Download size={16} />, tone: "success", run: onDownload },
    onDelete && { key: "delete", label: "Delete", icon: <Trash2 size={16} />, tone: "danger", run: onDelete },
  ].filter(Boolean) as { key: string; label: string; icon: React.ReactNode; tone: string; run: () => void }[];

  const swipe = useSwipeReveal({ actionsWidth: actions.length * ACTION_WIDTH });

  const face = (
    <button type="button" className="xplm-row-face" onClick={() => onOpen(entry)}>
      {entry.is_dir
        ? <span className="xplm-folder" aria-hidden="true"><Folder size={17} /></span>
        : <FileTile name={entry.name} />}
      <span className="xplm-row-text">
        <span className="xplm-row-name">
          {query ? <MatchedName name={entry.name} query={query} /> : entry.name}
        </span>
        {subtitle
          ? <span className="xplm-row-sub is-path">{subtitle}</span>
          : !entry.is_dir && <span className="xplm-row-sub">{formatSize(entry.size)}</span>}
      </span>
      {entry.is_dir && <ChevronRight size={16} className="xplm-row-chevron" aria-hidden="true" />}
    </button>
  );

  if (actions.length === 0) {
    return <div className="xplm-row">{face}</div>;
  }

  return (
    <div className={`xplm-row ${swipe.containerClass}`} {...swipe.handlers}>
      <div className="xplm-swipe-actions">
        {actions.map((action) => (
          <button
            key={action.key}
            type="button"
            className={`xplm-swipe-btn is-${action.tone}`}
            aria-label={`${action.label} ${entry.name}`}
            onClick={() => { swipe.close(); action.run(); }}
          >
            {action.icon}
            <span>{action.label}</span>
          </button>
        ))}
      </div>
      <div className="xplm-row-content" style={swipe.contentStyle}>
        {face}
      </div>
    </div>
  );
}
