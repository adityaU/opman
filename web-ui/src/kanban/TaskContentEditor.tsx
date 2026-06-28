import React, { useRef, useState, useCallback } from "react";
import { Video, ImagePlus } from "lucide-react";
import { uploadAttachment, assetUrl, type Attachment } from "../api/kanban";

interface Props {
  value: string;
  onChange: (value: string) => void;
  /** The task id to attach to. Null when the task hasn't been saved yet. */
  taskId: string | null;
  /** Notified when an upload completes (so callers can track attachments). */
  onAttached?: (att: Attachment) => void;
}

/** Markdown editor supporting paste/drag-drop image and a video upload button.
 *  On attach it POSTs to the attachment endpoint and inserts a markdown image
 *  (`![name](url)`) or an HTML `<video>` tag at the cursor. */
export const TaskContentEditor: React.FC<Props> = function TaskContentEditor(p) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);
  const [uploadError, setUploadError] = useState<string | null>(null);

  const insertAtCursor = useCallback(
    (snippet: string) => {
      const ta = textareaRef.current;
      const current = p.value;
      if (!ta) {
        p.onChange(current ? `${current}\n${snippet}` : snippet);
        return;
      }
      const start = ta.selectionStart ?? current.length;
      const end = ta.selectionEnd ?? current.length;
      const next = current.slice(0, start) + snippet + current.slice(end);
      p.onChange(next);
      // Restore cursor after the inserted snippet on next tick.
      requestAnimationFrame(() => {
        ta.focus();
        const pos = start + snippet.length;
        ta.setSelectionRange(pos, pos);
      });
    },
    [p],
  );

  const handleUpload = useCallback(
    async (file: File) => {
      if (!p.taskId) {
        setUploadError("Save the task first, then attach media.");
        return;
      }
      setUploading(true);
      setUploadError(null);
      try {
        const att = await uploadAttachment(p.taskId, file);
        const url = att.url || assetUrl(p.taskId, att.filename);
        const snippet =
          att.kind === "video"
            ? `\n<video src="${url}" controls></video>\n`
            : `\n![${att.filename}](${url})\n`;
        insertAtCursor(snippet);
        p.onAttached?.(att);
      } catch (e) {
        setUploadError(e instanceof Error ? e.message : "Upload failed");
      } finally {
        setUploading(false);
      }
    },
    [p, insertAtCursor],
  );

  const handlePaste = useCallback(
    (e: React.ClipboardEvent) => {
      const items = e.clipboardData?.items;
      if (!items) return;
      for (const item of items) {
        if (item.kind === "file" && item.type.startsWith("image/")) {
          const file = item.getAsFile();
          if (file) {
            e.preventDefault();
            handleUpload(file);
            return;
          }
        }
      }
    },
    [handleUpload],
  );

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      const files = e.dataTransfer?.files;
      if (!files || files.length === 0) return;
      const file = files[0];
      if (file.type.startsWith("image/") || file.type.startsWith("video/")) {
        e.preventDefault();
        handleUpload(file);
      }
    },
    [handleUpload],
  );

  return (
    <div className="kanban-content-editor">
      <div className="kanban-content-toolbar">
        <button
          type="button"
          className="kanban-content-tool-btn"
          disabled={!p.taskId || uploading}
          title={p.taskId ? "Upload image" : "Save the task first to attach media"}
          onClick={() => {
            if (fileInputRef.current) {
              fileInputRef.current.accept = "image/*";
              fileInputRef.current.click();
            }
          }}
        >
          <ImagePlus size={13} />
          <span>Image</span>
        </button>
        <button
          type="button"
          className="kanban-content-tool-btn"
          disabled={!p.taskId || uploading}
          title={p.taskId ? "Upload video" : "Save the task first to attach media"}
          onClick={() => {
            if (fileInputRef.current) {
              fileInputRef.current.accept = "video/*";
              fileInputRef.current.click();
            }
          }}
        >
          <Video size={13} />
          <span>Video</span>
        </button>
        {uploading && <span className="kanban-content-hint">Uploading…</span>}
        {!p.taskId && (
          <span className="kanban-content-hint">Save the task to enable attachments.</span>
        )}
        {uploadError && <span className="kanban-content-error">{uploadError}</span>}
      </div>

      <textarea
        ref={textareaRef}
        className="kanban-content-textarea"
        value={p.value}
        onChange={(e) => p.onChange(e.target.value)}
        onPaste={handlePaste}
        onDrop={handleDrop}
        onDragOver={(e) => {
          if (p.taskId) e.preventDefault();
        }}
        placeholder="Describe the task in markdown. Paste or drop an image, or use the buttons above…"
        rows={10}
        spellCheck
      />

      <input
        ref={fileInputRef}
        type="file"
        style={{ display: "none" }}
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) handleUpload(file);
          e.target.value = "";
        }}
      />
    </div>
  );
};
