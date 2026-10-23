import { useState, useEffect, useMemo } from "react";
import DOMPurify from "dompurify";
import mermaid from "mermaid";

export function MermaidViewer({ content }: { content: string }) {
  const [svg, setSvg]     = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const id = `mermaid-${Math.random().toString(36).slice(2, 10)}`;
    mermaid.initialize({ startOnLoad: false, theme: "base", securityLevel: "strict" });
    mermaid.render(id, content)
      .then((result) => { if (active) { setSvg(result.svg); setError(null); } })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err.message : "Failed to render Mermaid diagram");
          setSvg("");
        }
      });
    return () => { active = false; };
  }, [content]);

  const sanitizedSvg = useMemo(() => DOMPurify.sanitize(svg), [svg]);
  if (error) return <div className="file-preview file-preview-binary">{error}</div>;
  return <div className="mermaid-viewer" dangerouslySetInnerHTML={{ __html: sanitizedSvg }} />;
}
