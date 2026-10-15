/**
 * Turning a selection from an assistant reply into part of the user's next message.
 *
 * The selection becomes a Markdown quote — every runner reads `>` as "this is what I am
 * talking about" — followed by the user's own words. Very long selections are cut, so
 * selecting a whole reply by accident does not paste pages into a prompt.
 */

/** Most characters of a selection carried into a message. */
export const MAX_QUOTE_CHARS = 4000;

/** Window event asking a composer to add text; `detail` is a {@link ComposerInsert}. */
export const COMPOSER_INSERT = "opman:composer-insert";

export interface ComposerInsert {
  /** The session whose composer should take it; null for a new-session composer. */
  readonly sessionId: string | null;
  readonly text: string;
}

export function quote(selection: string): string {
  const trimmed = selection.trim();
  const cut = trimmed.length > MAX_QUOTE_CHARS ? `${trimmed.slice(0, MAX_QUOTE_CHARS)}…` : trimmed;
  return cut
    .split(/\r?\n/)
    .map((line) => (line ? `> ${line}` : ">"))
    .join("\n");
}

/** The message sent for "Ask": the quote, a blank line, then the question. */
export function askMessage(selection: string, question: string): string {
  return `${quote(selection)}\n\n${question.trim()}`;
}

/** Hand text to the composer of `sessionId`, which appends it and takes focus. */
export function insertIntoComposer(sessionId: string | null, text: string): void {
  window.dispatchEvent(new CustomEvent<ComposerInsert>(COMPOSER_INSERT, { detail: { sessionId, text } }));
}
