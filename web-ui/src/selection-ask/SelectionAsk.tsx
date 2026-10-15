import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { CornerDownLeft, MessageSquareQuote, TextQuote, X } from "lucide-react";
import { askMessage, insertIntoComposer, quote } from "./quote";
import { useAssistantSelection } from "./useAssistantSelection";

export interface SelectionAskProps {
  /** The timeline whose assistant replies can be quoted. */
  readonly containerRef: React.RefObject<HTMLElement | null>;
  readonly sessionId: string | null;
  /** Send a user message to this session. Without it only "Quote" is offered. */
  readonly onSend?: (text: string) => void;
}

/** Room kept between the bar and the viewport edge. */
const EDGE = 8;
const GAP = 8;

/**
 * A small bar over text selected in an assistant reply: ask about it right there, or
 * quote it into the composer and keep writing.
 *
 * "Ask" sends the selection as a quote plus the question, as a normal user message on
 * this session. Portalled to the body so no scrolling ancestor clips it, and drawn on the
 * shared popover surface so it follows the glassy and flat appearances.
 */
export function SelectionAsk({ containerRef, sessionId, onSend }: SelectionAskProps) {
  const [asking, setAsking] = useState(false);
  const [question, setQuestion] = useState("");
  const [selection, clear] = useAssistantSelection(containerRef, asking);
  const barRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{ top: number; left: number } | null>(null);

  const close = () => {
    setAsking(false);
    setQuestion("");
    clear();
  };

  // Above the selection, or below it when there is no room; centred and kept on screen.
  useLayoutEffect(() => {
    const bar = barRef.current;
    if (!selection || !bar) {
      setPosition(null);
      return;
    }
    const { width, height } = bar.getBoundingClientRect();
    const above = selection.rect.top - height - GAP;
    const top = above >= EDGE ? above : selection.rect.bottom + GAP;
    const centre = selection.rect.left + selection.rect.width / 2;
    const left = Math.min(Math.max(centre - width / 2, EDGE), window.innerWidth - width - EDGE);
    setPosition({ top, left });
  }, [selection, asking]);

  useEffect(() => {
    if (!selection) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selection]);

  if (!selection) return null;

  const send = () => {
    if (!question.trim() || !onSend) return;
    onSend(askMessage(selection.text, question));
    window.getSelection()?.removeAllRanges();
    close();
  };
  const quoteIt = () => {
    insertIntoComposer(sessionId, `${quote(selection.text)}\n\n`);
    window.getSelection()?.removeAllRanges();
    close();
  };

  return createPortal(
    <div
      ref={barRef}
      className="modal-popover-surface sel-ask"
      role="toolbar"
      aria-label="Selected text"
      style={position ? { top: position.top, left: position.left } : { visibility: "hidden" }}
      // Keep the page selection while a button is pressed; the field may still take focus.
      onMouseDown={(event) => {
        if (!(event.target instanceof HTMLInputElement)) event.preventDefault();
      }}
    >
      {asking ? (
        <form className="sel-ask-form" onSubmit={(event) => { event.preventDefault(); send(); }}>
          <input
            className="sel-ask-input"
            autoFocus
            value={question}
            placeholder="Ask about this…"
            aria-label="Question about the selected text"
            onChange={(event) => setQuestion(event.target.value)}
          />
          <button type="submit" className="sel-ask-btn sel-ask-btn--primary" disabled={!question.trim()}
            aria-label="Send question">
            <CornerDownLeft size={13} />
          </button>
        </form>
      ) : (
        <>
          {onSend && (
            <button type="button" className="sel-ask-btn" onClick={() => setAsking(true)}>
              <MessageSquareQuote size={13} /> Ask
            </button>
          )}
          <button type="button" className="sel-ask-btn" onClick={quoteIt}>
            <TextQuote size={13} /> Quote
          </button>
        </>
      )}
      <button type="button" className="sel-ask-icon" aria-label="Close" onClick={close}>
        <X size={13} />
      </button>
    </div>,
    document.body,
  );
}
