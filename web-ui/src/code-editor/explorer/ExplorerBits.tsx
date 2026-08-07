/**
 * ExplorerBits — the small pieces every explorer row is assembled from:
 * the typed file tile, the inline name field, and the inline delete confirm.
 */
import { useEffect, useRef, useState } from "react";
import { ChevronRight, Loader2 } from "lucide-react";
import { fileGlyph } from "./fileGlyph";

/** Indentation step, in px. Kept here so tree and finder agree. */
export const INDENT = 13;

// ── Typed file tile ─────────────────────────────────────

export function FileTile({ name }: { name: string }) {
  const { mark, hue } = fileGlyph(name);
  return (
    <span
      className="xpl-tile"
      style={{ "--tile-hue": hue } as React.CSSProperties}
      aria-hidden="true"
    >
      {mark}
    </span>
  );
}

export function DirTile({ open, loading }: { open: boolean; loading: boolean }) {
  return (
    <span className={`xpl-dir-tile${open ? " is-open" : ""}`} aria-hidden="true">
      {loading ? <Loader2 size={11} className="spin" /> : <ChevronRight size={11} />}
    </span>
  );
}

// ── Name with the matched span emphasised ───────────────

export function MatchedName({ name, query }: { name: string; query: string }) {
  if (!query) return <>{name}</>;
  const at = name.toLowerCase().indexOf(query.toLowerCase());
  if (at < 0) return <>{name}</>;
  return (
    <>
      {name.slice(0, at)}
      <mark className="xpl-match">{name.slice(at, at + query.length)}</mark>
      {name.slice(at + query.length)}
    </>
  );
}

// ── Inline name field (create + rename share it) ────────

interface NameFieldProps {
  initial: string;
  placeholder?: string;
  /** Files preselect the stem so the extension survives a fast retype. */
  selectStem?: boolean;
  depth: number;
  icon: React.ReactNode;
  onSubmit: (name: string) => void;
  onCancel: () => void;
}

export function InlineNameField({
  initial, placeholder, selectStem, depth, icon, onSubmit, onCancel,
}: NameFieldProps) {
  const [value, setValue] = useState(initial);
  const ref = useRef<HTMLInputElement>(null);
  const settled = useRef(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.focus();
    if (!initial) return;
    const dot = selectStem ? initial.lastIndexOf(".") : -1;
    if (dot > 0) el.setSelectionRange(0, dot);
    else el.select();
  }, [initial, selectStem]);

  const submit = () => {
    if (settled.current) return;
    settled.current = true;
    const trimmed = value.trim();
    if (trimmed && trimmed !== initial) onSubmit(trimmed);
    else onCancel();
  };

  const cancel = () => {
    if (settled.current) return;
    settled.current = true;
    onCancel();
  };

  return (
    <div className="xpl-namefield" style={{ paddingLeft: `${8 + depth * INDENT}px` }}>
      {icon}
      <input
        ref={ref}
        className="xpl-namefield-input"
        value={value}
        placeholder={placeholder}
        spellCheck={false}
        autoComplete="off"
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") { e.preventDefault(); submit(); }
          if (e.key === "Escape") { e.preventDefault(); cancel(); }
        }}
        onBlur={submit}
      />
    </div>
  );
}

// ── Inline delete confirm ───────────────────────────────

export function ConfirmDelete({ name, isDir, depth, onConfirm, onCancel }: {
  name: string; isDir: boolean; depth: number;
  onConfirm: () => void; onCancel: () => void;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => { ref.current?.focus(); }, []);

  return (
    <div
      className="xpl-confirm"
      style={{ paddingLeft: `${8 + depth * INDENT}px` }}
      onKeyDown={(e) => { if (e.key === "Escape") onCancel(); }}
    >
      <span className="xpl-confirm-text">
        Delete {isDir ? "folder" : "file"} <strong>{name}</strong>
        {isDir ? " and everything inside it?" : "?"}
      </span>
      <span className="xpl-confirm-actions">
        <button type="button" className="xpl-confirm-no" onClick={onCancel}>Keep</button>
        <button ref={ref} type="button" className="xpl-confirm-yes" onClick={onConfirm}>Delete</button>
      </span>
    </div>
  );
}
