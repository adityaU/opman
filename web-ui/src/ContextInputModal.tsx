import React, { useState, useRef, useEffect, useCallback } from "react";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { FileText, X } from "lucide-react";

interface Props {
  onClose: () => void;
  onSubmit: (text: string) => void;
}

export function ContextInputModal({ onClose, onSubmit }: Props) {
  const [text, setText] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useFocusTrap(modalRef);

  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  // Auto-resize textarea
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 400)}px`;
  }, [text]);

  const handleSubmit = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed) return;
    onSubmit(trimmed);
    onClose();
  }, [text, onSubmit, onClose]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Ctrl+D / Cmd+D to submit
      if (e.key === "d" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        handleSubmit();
        return;
      }
      // Escape to close
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }
      // Enter inserts a newline (default behavior)
      // Ctrl+Enter also submits
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        handleSubmit();
        return;
      }
    },
    [handleSubmit, onClose]
  );

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="context-input-modal" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Context input" ref={modalRef}>
        {/* Header */}
        <div className="context-input-header">
          <FileText size={14} />
          <div className="context-input-titles">
            <span className="context-input-title">Context Input</span>
            <span className="context-input-subtitle">
              Insert context for the AI session
            </span>
          </div>
          <button className="context-input-close" onClick={onClose} aria-label="Close context input">
            <X size={14} />
          </button>
        </div>

        {/* Text area */}
        <div className="context-input-body">
          <textarea
            ref={textareaRef}
            className="context-input-textarea"
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type context, instructions, or code snippets..."
            rows={8}
          />
        </div>

        {/* Footer */}
        <div className="context-input-footer">
          <div className="context-input-hints">
            <kbd>Enter</kbd> Newline
            <kbd>Cmd+Enter</kbd> Submit
            <kbd>Cmd+D</kbd> Submit
            <kbd>Esc</kbd> Cancel
          </div>
          <button
            className="context-input-submit"
            onClick={handleSubmit}
            disabled={!text.trim()}
          >
            Send Context
          </button>
        </div>
      </div>
    </div>
  );
}
