/**
 * A unified diff, rendered from the raw text git gives us.
 *
 * Parsing is deliberately shallow: git's own line prefixes already carry every
 * distinction the reader needs (file, hunk, added, removed, context), so the
 * component classifies lines rather than reconstructing a file model. That
 * keeps a truncated or unusual diff renderable instead of throwing.
 */

import { useMemo } from "react";

type DiffKind = "file" | "hunk" | "add" | "del" | "meta" | "context";

interface DiffLine {
  kind: DiffKind;
  marker: string;
  text: string;
}

const META_PREFIXES = [
  "index ",
  "new file",
  "deleted file",
  "old mode",
  "new mode",
  "similarity index",
  "rename from",
  "rename to",
  "copy from",
  "copy to",
  "--- ",
  "+++ ",
];

function classify(line: string): DiffLine {
  if (line.startsWith("diff --git")) return { kind: "file", marker: "", text: line.slice(11) };
  if (line.startsWith("@@")) return { kind: "hunk", marker: "", text: line };
  if (META_PREFIXES.some((prefix) => line.startsWith(prefix))) {
    return { kind: "meta", marker: "", text: line };
  }
  if (line.startsWith("+")) return { kind: "add", marker: "+", text: line.slice(1) };
  if (line.startsWith("-")) return { kind: "del", marker: "-", text: line.slice(1) };
  if (line.startsWith("\\")) return { kind: "meta", marker: "", text: line };
  return { kind: "context", marker: " ", text: line.startsWith(" ") ? line.slice(1) : line };
}

const KIND_CLASS: Record<DiffKind, string> = {
  file: "gitp-diff-file",
  hunk: "gitp-diff-hunk",
  add: "gitp-diff-add",
  del: "gitp-diff-del",
  meta: "gitp-diff-meta",
  context: "gitp-diff-context",
};

export interface DiffViewProps {
  diff: string;
  /** Shown instead of the generic empty copy when the caller knows better. */
  emptyLabel?: string;
}

export function DiffView({ diff, emptyLabel }: DiffViewProps) {
  const lines = useMemo(() => {
    const trimmed = diff.replace(/\n$/, "");
    if (!trimmed.trim()) return [];
    return trimmed.split("\n").map(classify);
  }, [diff]);

  const binary = useMemo(() => /^Binary files? /m.test(diff), [diff]);

  if (binary) {
    return (
      <div className="gitp-diff gitp-diff-binary">
        <p className="gitp-diff-note">Binary file — no text diff</p>
      </div>
    );
  }

  if (lines.length === 0) {
    return (
      <div className="gitp-diff gitp-diff-empty">
        <p className="gitp-diff-note">{emptyLabel ?? "Nothing to show — this file has no textual changes."}</p>
      </div>
    );
  }

  return (
    <div className="gitp-diff">
      <pre className="gitp-diff-code">
        <code>
          {lines.map((line, index) => (
            <span key={index} className={`gitp-diff-line ${KIND_CLASS[line.kind]}`}>
              <span className="gitp-diff-marker" aria-hidden="true">
                {line.marker}
              </span>
              <span className="gitp-diff-text">{line.text || " "}</span>
            </span>
          ))}
        </code>
      </pre>
    </div>
  );
}
