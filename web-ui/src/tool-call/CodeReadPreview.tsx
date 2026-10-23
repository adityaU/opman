import React from "react";
import { chooseViewer, ReadViewer } from "./read-viewer";
import type { InnerCall } from "./codeModeParse";
import { SyntaxHighlighter } from "../utils/syntax";
import { guessLanguage, parseOutput } from "./helpers";
import { withReadableOutput } from "./opmanTools";
import { EditDiffView } from "./EditDiffView";

const STYLE = { margin: 0, maxHeight: 300, overflow: "auto", fontFamily: "var(--font-mono)" };

function extractOutputDiff(outputText: string): string | undefined {
  try {
    const parsed: unknown = JSON.parse(outputText);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const diff = (parsed as Record<string, unknown>).diff;
      if (typeof diff === "string" && diff.length > 0) return diff;
    }
  } catch { /* ignore */ }
  return undefined;
}

export function hasCodePreview(call: InnerCall): boolean {
  const read = /(?:^|[._])(?:read|read_file|view_file|view_image|open_file|cat)$/i.test(call.tool);
  if (!read || !call.ok || !call.output.text) return false;
  try {
    const input: unknown = JSON.parse(call.input.text);
    if (!input || typeof input !== "object" || Array.isArray(input)) return false;
    const args = input as Record<string, unknown>;
    const path = args.path ?? args.file_path ?? args.filePath ?? args.filename ?? args.absolute_path;
    return typeof path === "string" && path.length > 0;
  } catch {
    return false;
  }
}

/** Read calls inside execute keep the same file viewers as standalone tools. */
export function CodeReadPreview({ call }: { call: InnerCall }) {
  const read = /(?:^|[._])(?:read|read_file|view_file|view_image|open_file|cat)$/i.test(call.tool);
  const edit = /(?:^|[._])(?:edit|edit_file|write|write_file|multi_edit|multiedit)$/i.test(call.tool);
  if (!read && !edit) return null;
  try {
    const input: unknown = JSON.parse(call.input.text);
    if (!input || typeof input !== "object" || Array.isArray(input)) return null;
    const args = input as Record<string, unknown>;
    if (edit) return <EditDiffView input={args} outputDiff={extractOutputDiff(call.output.text)} />;
    const path = args.path ?? args.file_path ?? args.filePath ?? args.filename ?? args.absolute_path;
    if (typeof path !== "string") return null;
    const viewer = chooseViewer(path, call.output.text);
    if (viewer) return <ReadViewer path={path} kind={viewer.kind} source={viewer.source} />;
    if (!call.ok || !call.output.text) return null;
    const part = withReadableOutput({ type: "tool", tool: call.tool.replace("opman_tools.", "opman_tools_"),
      state: { status: "completed", input: args, output: call.output.text } });
    const parsed = parseOutput(part.state?.output || call.output.text);
    return <SyntaxHighlighter language={guessLanguage(path)} useInlineStyles={false}
      PreTag="div" customStyle={STYLE}>{parsed.content}</SyntaxHighlighter>;
  } catch {
    return null;
  }
}
