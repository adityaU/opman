import React, { lazy, Suspense } from "react";
import { CsvViewer, MarkdownViewer, SvgViewer } from "../content-viewers/MarkupViewers";
import { HtmlViewer } from "../content-viewers/HtmlViewer";
import type { CodePreviewKind } from "./codePreviewKind";

const MermaidViewer = lazy(() => import("../content-viewers/MermaidViewer")
  .then(module => ({ default: module.MermaidViewer })));

export function CodePreview({ kind, code }: { kind: CodePreviewKind; code: string }) {
  switch (kind) {
    case "markdown": return <MarkdownViewer content={code} />;
    case "html": return <HtmlViewer content={code} />;
    case "svg": return <SvgViewer content={code} />;
    case "csv": return <CsvViewer content={code} />;
    case "mermaid": return <Suspense fallback={<p role="status">Rendering…</p>}>
      <MermaidViewer content={code} />
    </Suspense>;
  }
}
