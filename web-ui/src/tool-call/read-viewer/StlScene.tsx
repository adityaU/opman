import React, { useEffect, useRef, useState } from "react";
import { Loading, Unavailable } from "./Unavailable";
import { MAX_STL_BYTES } from "./stlLimits";
import { formatBytes } from "./fileKind";
import type { StlViewerProps } from "./StlViewer";

type Phase = "loading" | "ready" | "failed" | "too-big";

/** Orbit viewer for ASCII and binary STL. Renders only when the camera moves. */
export default function StlScene({ url, href, onMeta }: StlViewerProps) {
  const host = useRef<HTMLDivElement>(null);
  const [phase, setPhase] = useState<Phase>("loading");

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let disposed = false;
    let cleanup: () => void = () => {};
    const abort = new AbortController();
    setPhase("loading");

    (async () => {
      const res = await fetch(url, { signal: abort.signal });
      if (!res.ok) throw new Error("missing");
      const declared = Number(res.headers.get("content-length"));
      if (declared > MAX_STL_BYTES) { setPhase("too-big"); return; }
      const bytes = await res.arrayBuffer();
      if (disposed) return;
      if (bytes.byteLength > MAX_STL_BYTES) { setPhase("too-big"); return; }
      const { buildScene } = await import("./stlBuild");
      if (disposed) return;
      const built = buildScene(el, bytes);
      cleanup = built.dispose;
      onMeta(`${formatBytes(bytes.byteLength)} · ${built.triangles.toLocaleString()} triangles`);
      setPhase("ready");
    })().catch(() => { if (!disposed) setPhase("failed"); });

    return () => { disposed = true; abort.abort(); cleanup(); };
  }, [url, onMeta]);

  return (
    <div className="rv-stl-wrap">
      {phase === "loading" && <Loading label="Loading model…" />}
      {phase === "failed" && <Unavailable message="Could not load this model" href={href} />}
      {phase === "too-big" && (
        <Unavailable message={`Model is over ${formatBytes(MAX_STL_BYTES)}`} href={href} />
      )}
      <div ref={host} className={`rv-stl${phase === "ready" ? "" : " rv-hidden"}`} />
    </div>
  );
}
