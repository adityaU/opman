import React, { lazy, Suspense } from "react";
import { Loading, Unavailable } from "./Unavailable";
import { useFileProbe } from "./useFileProbe";
import { formatBytes } from "./fileKind";

const PdfCanvasViewer = lazy(() => import("../../code-editor/components/PdfCanvasViewer")
  .then(module => ({ default: module.PdfCanvasViewer })));

/** Reuse the editor's PDF renderer, including on browsers without a PDF plugin. */
export function PdfViewer({ url, name, onMeta, href }: {
  readonly url: string; readonly name: string; readonly href: string | null;
  readonly onMeta: (text: string) => void;
}) {
  const probe = useFileProbe(url);
  React.useEffect(() => {
    if (probe.state === "ready" && probe.size) onMeta(formatBytes(probe.size));
  }, [probe, onMeta]);

  if (probe.state === "checking") return <Loading />;
  if (probe.state === "missing") return <Unavailable href={href} />;
  return (
    <div className="rv-pdf-wrap">
      <Suspense fallback={<Loading label="Loading PDF viewer…" />}>
        <PdfCanvasViewer url={url} alt={name} />
      </Suspense>
    </div>
  );
}
