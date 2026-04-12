/**
 * DocumentEditor — contenteditable HTML editing with a formatting toolbar
 * for bold, italic, underline, headings, and lists.
 * Matches Leptos document_editor.rs.
 */
import { useCallback, useRef } from "react";
import { File } from "lucide-react";
import type { DocData, OpenFileEntry } from "../types";

interface Props {
  path: string;
  docData: DocData;
  setOpenFiles: React.Dispatch<React.SetStateAction<OpenFileEntry[]>>;
}

function execCommand(cmd: string, value = "") {
  document.execCommand(cmd, false, value);
}

export function DocumentEditor({ path, docData, setOpenFiles }: Props) {
  if (docData.type !== "document") {
    return (
      <div className="file-preview file-preview-binary">
        <span>Not a document</span>
      </div>
    );
  }

  const { html } = docData;
  if (!html.trim()) {
    return (
      <div className="file-preview file-preview-binary">
        <File size={48} strokeWidth={1} />
        <span>Empty document</span>
      </div>
    );
  }

  const contentRef = useRef<HTMLDivElement>(null);

  const handleInput = useCallback(() => {
    const el = contentRef.current;
    if (!el) return;
    const newHtml = el.innerHTML;
    setOpenFiles((prev) =>
      prev.map((f) =>
        f.path === path
          ? { ...f, editedDocData: { type: "document", html: newHtml } }
          : f,
      ),
    );
  }, [path, setOpenFiles]);

  return (
    <div className="document-viewer document-viewer-editable">
      <div className="document-toolbar">
        <button className="doc-toolbar-btn" title="Bold (Ctrl+B)" onClick={() => execCommand("bold")}>B</button>
        <button className="doc-toolbar-btn doc-toolbar-italic" title="Italic (Ctrl+I)" onClick={() => execCommand("italic")}>I</button>
        <button className="doc-toolbar-btn doc-toolbar-underline" title="Underline (Ctrl+U)" onClick={() => execCommand("underline")}>U</button>
        <button className="doc-toolbar-btn doc-toolbar-strike" title="Strikethrough" onClick={() => execCommand("strikeThrough")}>S</button>
        <span className="doc-toolbar-sep" />
        <button className="doc-toolbar-btn" title="Heading 1" onClick={() => execCommand("formatBlock", "h1")}>H1</button>
        <button className="doc-toolbar-btn" title="Heading 2" onClick={() => execCommand("formatBlock", "h2")}>H2</button>
        <button className="doc-toolbar-btn" title="Heading 3" onClick={() => execCommand("formatBlock", "h3")}>H3</button>
        <button className="doc-toolbar-btn" title="Paragraph" onClick={() => execCommand("formatBlock", "p")}>P</button>
        <span className="doc-toolbar-sep" />
        <button className="doc-toolbar-btn" title="Bulleted list" onClick={() => execCommand("insertUnorderedList")}>UL</button>
        <button className="doc-toolbar-btn" title="Numbered list" onClick={() => execCommand("insertOrderedList")}>OL</button>
      </div>
      <div
        ref={contentRef}
        className="document-content"
        contentEditable
        suppressContentEditableWarning
        dangerouslySetInnerHTML={{ __html: html }}
        onInput={handleInput}
      />
    </div>
  );
}
