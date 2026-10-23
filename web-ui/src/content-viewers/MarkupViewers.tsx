import { useMemo } from "react";
import DOMPurify from "dompurify";
import ReactMarkdown from "react-markdown";
import { markdownComponents, REMARK_PLUGINS } from "../message-turn/CodeBlock";

export function CsvViewer({ content }: { content: string }) {
  const rows = useMemo(() => {
    if (!content.trim()) return [];
    return content.split("\n").map((line) => {
      const cells: string[] = [];
      let current = "";
      let inQuotes = false;
      for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (ch === '"') {
          if (inQuotes && line[i + 1] === '"') { current += '"'; i++; }
          else inQuotes = !inQuotes;
        } else if (ch === "," && !inQuotes) {
          cells.push(current.trim()); current = "";
        } else {
          current += ch;
        }
      }
      cells.push(current.trim());
      return cells;
    });
  }, [content]);

  if (rows.length === 0) {
    return <div className="file-preview file-preview-binary"><span>Empty CSV file</span></div>;
  }

  const header = rows[0];
  const body = rows.slice(1).filter((r) => r.some((c) => c.length > 0));

  return (
    <div className="csv-viewer">
      <table>
        <thead><tr>{header.map((cell, i) => <th key={i}>{cell}</th>)}</tr></thead>
        <tbody>{body.map((row, ri) => <tr key={ri}>{row.map((cell, ci) => <td key={ci}>{cell}</td>)}</tr>)}</tbody>
      </table>
    </div>
  );
}

// ── Markdown ────────────────────────────────────────────

export function MarkdownViewer({ content }: { content: string }) {
  return (
    <div className="markdown-viewer">
      <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>{content}</ReactMarkdown>
    </div>
  );
}

export function SvgViewer({ content }: { content: string }) {
  const sanitized = useMemo(
    () => DOMPurify.sanitize(content, { USE_PROFILES: { svg: true, svgFilters: true } }),
    [content],
  );
  const source = useMemo(() => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(sanitized)}`, [sanitized]);
  return <div className="svg-viewer"><img src={source} alt="SVG preview" /></div>;
}
