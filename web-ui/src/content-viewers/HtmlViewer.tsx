import { useEffect, useMemo, useState } from "react";
import DOMPurify from "dompurify";
import { THEME_CHANGED_EVENT } from "../utils/theme";

function themeStyle(): string {
  const styles = getComputedStyle(document.documentElement);
  const colour = (name: string) => styles.getPropertyValue(name).trim();
  // Read colour values only; reject strings that could end the style element.
  const safe = (value: string) => /^[#\w\s.,()%/+-]+$/.test(value) ? value : "inherit";
  return `:root{color-scheme:${document.documentElement.classList.contains("light-theme") ? "light" : "dark"}}
    body{margin:16px;background:${safe(colour("--color-bg-panel"))};color:${safe(colour("--color-text"))};font:14px/1.6 system-ui,sans-serif}
    a{color:${safe(colour("--color-secondary"))}}img,svg,video{max-width:100%}`;
}

/** Sandboxed document preview shared by the file browser and chat code blocks. */
export function HtmlViewer({ content }: { content: string }) {
  const [theme, setTheme] = useState(themeStyle);
  useEffect(() => {
    const changed = () => setTheme(themeStyle());
    window.addEventListener(THEME_CHANGED_EVENT, changed);
    return () => window.removeEventListener(THEME_CHANGED_EVENT, changed);
  }, []);
  const sanitized = useMemo(() => DOMPurify.sanitize(content, { WHOLE_DOCUMENT: true }), [content]);
  const document = useMemo(() => sanitized.replace(/<head(?:\s[^>]*)?>/i,
    head => `${head}<style>${theme}</style>`), [sanitized, theme]);
  return <iframe className="html-viewer-frame" sandbox="" srcDoc={document} title="HTML preview" />;
}
