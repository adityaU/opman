import React from "react";
import { chooseViewer, ReadViewer } from "./read-viewer";
import type { InnerCall } from "./codeModeParse";

/** Read calls inside execute keep the same file viewers as standalone tools. */
export function CodeReadPreview({ call }: { call: InnerCall }) {
  if (!/(?:^|[._])(?:read|read_file|view_image|open_file|cat)$/i.test(call.tool)) return null;
  try {
    const input: unknown = JSON.parse(call.input.text);
    if (!input || typeof input !== "object" || Array.isArray(input)) return null;
    const args = input as Record<string, unknown>;
    const path = args.path ?? args.file_path ?? args.filePath ?? args.filename ?? args.absolute_path;
    if (typeof path !== "string") return null;
    const viewer = chooseViewer(path, call.output.text);
    if (!viewer) return null;
    return <ReadViewer path={path} kind={viewer.kind} source={viewer.source} />;
  } catch {
    return null;
  }
}
