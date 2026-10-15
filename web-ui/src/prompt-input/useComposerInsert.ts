import { useEffect, useRef } from "react";
import { COMPOSER_INSERT, type ComposerInsert } from "../selection-ask/quote";

/**
 * Accept text another surface hands this composer — a quote from an assistant reply —
 * when it is addressed to this composer's session. The text is added after whatever is
 * already typed, the field takes focus, and the caret goes to the end, ready for the
 * user's own words.
 */
export function useComposerInsert(
  sessionId: string | null,
  setText: (update: (current: string) => string) => void,
  textareaRef: React.RefObject<HTMLTextAreaElement | null>,
  onContentChange?: (hasContent: boolean) => void,
): void {
  const latest = useRef({ sessionId, setText, onContentChange });
  latest.current = { sessionId, setText, onContentChange };

  useEffect(() => {
    const insert = (event: Event) => {
      const { sessionId: target, text } = (event as CustomEvent<ComposerInsert>).detail;
      const current = latest.current;
      if (target !== current.sessionId) return;
      current.setText((typed) => (typed.trim() ? `${typed.replace(/\s*$/, "")}\n\n${text}` : text));
      current.onContentChange?.(true);
      requestAnimationFrame(() => {
        const field = textareaRef.current;
        if (!field) return;
        field.focus();
        field.setSelectionRange(field.value.length, field.value.length);
      });
    };
    window.addEventListener(COMPOSER_INSERT, insert);
    return () => window.removeEventListener(COMPOSER_INSERT, insert);
  }, [textareaRef]);
}
