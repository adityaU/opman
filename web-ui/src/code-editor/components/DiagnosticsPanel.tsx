/**
 * DiagnosticsPanel — the file's problems, in one place you can work through.
 *
 * Underlines tell you a token is wrong; they do not tell you how many problems
 * the file has, or let you get to the one on line 400 without scrolling to it.
 * This is the other half: a count you can see without hunting, a list ordered
 * the way you would read the file, and a click that takes you to the line.
 *
 * It stays collapsed by default — a panel that opens itself every time a
 * half-typed line is briefly invalid would push the code around while you type.
 * The header alone carries enough to decide whether to open it.
 */
import { useMemo, useState } from "react";
import { AlertCircle, AlertTriangle, ChevronDown, Info, CheckCircle2 } from "lucide-react";
import type { EditorLspDiagnostic } from "../types";

interface Props {
  diagnostics: EditorLspDiagnostic[];
  /** Null while the language server for this file is unavailable. */
  available: boolean;
  onJump: (line: number, col: number) => void;
}

type Severity = "error" | "warning" | "info" | "hint";

function severityOf(diagnostic: EditorLspDiagnostic): Severity {
  const raw = (diagnostic.severity || "").toLowerCase();
  if (raw.startsWith("warn")) return "warning";
  if (raw.startsWith("info")) return "info";
  if (raw.startsWith("hint")) return "hint";
  return "error";
}

const ICONS: Record<Severity, React.ReactNode> = {
  error: <AlertCircle size={13} />,
  warning: <AlertTriangle size={13} />,
  info: <Info size={13} />,
  hint: <Info size={13} />,
};

/** Errors first, then by position — the order you would work through them. */
const RANK: Record<Severity, number> = { error: 0, warning: 1, info: 2, hint: 3 };

export function DiagnosticsPanel({ diagnostics, available, onJump }: Props) {
  const [open, setOpen] = useState(false);

  const { sorted, errors, warnings } = useMemo(() => {
    const sorted = [...diagnostics].sort((a, b) => {
      const rank = RANK[severityOf(a)] - RANK[severityOf(b)];
      return rank !== 0 ? rank : a.lnum - b.lnum || a.col - b.col;
    });
    return {
      sorted,
      errors: diagnostics.filter((d) => severityOf(d) === "error").length,
      warnings: diagnostics.filter((d) => severityOf(d) === "warning").length,
    };
  }, [diagnostics]);

  if (!available) return null;

  const total = diagnostics.length;
  const clean = total === 0;

  return (
    <div className={`diagp${open ? " is-open" : ""}${clean ? " is-clean" : ""}`}>
      <button
        type="button"
        className="diagp-head"
        aria-expanded={open}
        onClick={() => !clean && setOpen((value) => !value)}
        disabled={clean}
      >
        {clean ? (
          <>
            <CheckCircle2 size={13} className="diagp-clean-icon" />
            <span className="diagp-label">No problems</span>
          </>
        ) : (
          <>
            <ChevronDown size={13} className="diagp-caret" />
            <span className="diagp-label">
              {total} {total === 1 ? "problem" : "problems"}
            </span>
            <span className="diagp-counts">
              {errors > 0 && (
                <span className="diagp-count is-error">
                  <AlertCircle size={11} />{errors}
                </span>
              )}
              {warnings > 0 && (
                <span className="diagp-count is-warning">
                  <AlertTriangle size={11} />{warnings}
                </span>
              )}
            </span>
          </>
        )}
      </button>

      {open && !clean && (
        <ul className="diagp-list">
          {sorted.map((diagnostic, index) => {
            const severity = severityOf(diagnostic);
            return (
              <li key={`${diagnostic.lnum}-${diagnostic.col}-${index}`}>
                <button
                  type="button"
                  className={`diagp-item is-${severity}`}
                  onClick={() => onJump(diagnostic.lnum, diagnostic.col)}
                >
                  <span className="diagp-item-icon">{ICONS[severity]}</span>
                  <span className="diagp-item-body">
                    <span className="diagp-item-msg">{diagnostic.message}</span>
                    {diagnostic.source && (
                      <span className="diagp-item-source">{diagnostic.source}</span>
                    )}
                  </span>
                  <span className="diagp-item-pos">
                    {diagnostic.lnum}:{diagnostic.col}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
