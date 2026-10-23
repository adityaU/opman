import React, { useMemo } from "react";
import { Minus, Plus } from "lucide-react";
import { ToolInput } from "./components";
import { guessLanguage } from "./helpers";
import { DiffHunk } from "./DiffHunk";
import { lineDiff } from "./diffRows";

const str = (value: unknown): string => typeof value === "string" ? value : "";
type Hunk = { oldStr: string; newStr: string };

function parseUnifiedDiff(diffText: string): Hunk[] {
  const lines = diffText.split(/\r?\n/);
  const hunks: Hunk[] = [];
  let oldLines: string[] = [];
  let newLines: string[] = [];
  let inHunk = false;
  const flush = () => {
    if (oldLines.length || newLines.length) {
      hunks.push({ oldStr: oldLines.join("\n"), newStr: newLines.join("\n") });
      oldLines = [];
      newLines = [];
    }
  };
  for (const line of lines) {
    if (line.startsWith("@@")) {
      flush();
      inHunk = true;
      continue;
    }
    if (!inHunk && (line.startsWith("---") || line.startsWith("+++") || line.startsWith("diff ") || line.startsWith("Edited "))) {
      continue;
    }
    if (line.startsWith("-")) {
      inHunk = true;
      oldLines.push(line.slice(1));
    } else if (line.startsWith("+")) {
      inHunk = true;
      newLines.push(line.slice(1));
    } else if (inHunk && line.startsWith(" ")) {
      oldLines.push(line.slice(1));
      newLines.push(line.slice(1));
    }
  }
  flush();
  return hunks;
}

export function EditDiffView({ input, outputDiff }: { input: Record<string, unknown> | string; outputDiff?: string }) {
  const parsed = useMemo(() => {
    try {
      const data: unknown = typeof input === "string" ? JSON.parse(input) : input;
      if (!data || typeof data !== "object" || Array.isArray(data)) return null;
      const args = data as Record<string, unknown>;
      const filePath = str(
        args.filePath ||
          args.file_path ||
          args.path ||
          args.filename ||
          args.absolute_path ||
          args.AbsolutePath ||
          args.TargetFile ||
          args.target_file
      );
      const pair = (edit: Record<string, unknown>): Hunk => ({
        oldStr: str(
          edit.oldString ??
            edit.old_string ??
            edit.oldText ??
            edit.old_text ??
            edit.TargetContent ??
            edit.target_content
        ),
        newStr: str(
          edit.newString ??
            edit.new_string ??
            edit.newText ??
            edit.new_text ??
            edit.ReplacementContent ??
            edit.replacement_content
        ),
      });
      const editList = Array.isArray(args.edits)
        ? args.edits
        : Array.isArray(args.ReplacementChunks)
        ? args.ReplacementChunks
        : Array.isArray(args.replacement_chunks)
        ? args.replacement_chunks
        : null;
      const contentText = args.content ?? args.CodeContent ?? args.code_content ?? args.file_text;
      let hunks = editList
        ? editList
            .filter((edit): edit is Record<string, unknown> => edit !== null && typeof edit === "object" && !Array.isArray(edit))
            .map(pair)
        : contentText !== undefined
        ? [{ oldStr: "", newStr: str(contentText) }]
        : [pair(args)];
      if (hunks.every((h) => !h.oldStr && !h.newStr)) {
        const diffSrc = str(args.diff) || str(outputDiff);
        if (diffSrc) hunks = parseUnifiedDiff(diffSrc);
      }
      let added = 0;
      let removed = 0;
      let total = 0;
      const highlightedHunks = hunks.map(hunk => ({ ...hunk, rows: lineDiff(hunk.oldStr, hunk.newStr) }));
      for (const hunk of highlightedHunks) {
        for (const row of hunk.rows) {
          total++;
          if (row.type === "add") added++;
          if (row.type === "del") removed++;
        }
      }
      return total ? { filePath, hunks: highlightedHunks, added, removed } : null;
    } catch {
      return null;
    }
  }, [input, outputDiff]);

  if (!parsed) return <ToolInput data={input} />;
  return <div className="diff-view tool-code-diff">
    {parsed.filePath && <div className="diff-header">
      <span className="diff-file-path">{parsed.filePath}</span>
    </div>}
    {parsed.hunks.map((hunk, index) =>
      <DiffHunk key={index} {...hunk} language={guessLanguage(parsed.filePath)} />)}
    <div className="diff-stats">
      <span className="diff-stats-removed"><Minus size={10} /> {parsed.removed} removed</span>
      <span className="diff-stats-added"><Plus size={10} /> {parsed.added} added</span>
    </div>
  </div>;
}
