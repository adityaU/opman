/**
 * lspCompletion — the completion source backing CodeMirror's autocomplete.
 *
 * Two details decide whether this feels native or merely present.
 *
 * The server's ranking is the ranking. rust-analyzer puts the field you are
 * almost certainly reaching for first via `sortText`; re-sorting alphabetically
 * on the client throws that away. CodeMirror sorts by its own score unless told
 * not to, so `boost` is derived from the server's order.
 *
 * And an *incomplete* list must be re-queried on every keystroke rather than
 * filtered down. A server that truncated its answer has not seen the rest of
 * what you are typing; narrowing its truncated list converges on nothing.
 */
import {
  autocompletion, completionStatus, moveCompletionSelection, snippetCompletion, startCompletion,
  type Completion, type CompletionContext, type CompletionResult,
} from "@codemirror/autocomplete";
import { Prec } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { markdownElement } from "./lspMarkdown";
import type { LspBridgeRef } from "./editorLsp";

export interface LspCompletionItem {
  label: string;
  kind: string;
  detail: string;
  documentation: string | null;
  insert: string;
  snippet: boolean;
  sort: string;
  filter: string;
  preselect: boolean;
  deprecated: boolean;
}

export interface LspCompletionResponse {
  available: boolean;
  items: LspCompletionItem[];
  incomplete: boolean;
  triggerCharacters: string[];
}

/** The word being typed, so CodeMirror knows what the completion replaces. */
const WORD_BEFORE = /[\w$@#.:>-]*$/;

function toCompletion(item: LspCompletionItem, index: number, total: number): Completion {
  const base: Completion = {
    label: item.label,
    type: item.deprecated ? "text" : item.kind,
    detail: item.detail || undefined,
    // Preserve the server's order: earlier items get a higher boost, and a
    // preselected item is lifted above everything.
    boost: (item.preselect ? 99 : 0) + Math.max(-99, Math.round(((total - index) / total) * 50)),
    info: item.documentation
      ? () => markdownElement(item.documentation as string, "cm-lsp-doc")
      : undefined,
  };

  if (item.snippet) {
    const completion = snippetCompletion(item.insert, base);
    return { ...completion, type: base.type, detail: base.detail, info: base.info };
  }
  if (item.insert === item.label) return base;
  return { ...base, apply: item.insert };
}

export function lspCompletionExtension(bridge: LspBridgeRef) {
  const source = async (context: CompletionContext): Promise<CompletionResult | null> => {
    const word = context.matchBefore(WORD_BEFORE);
    if (context.explicit) return documentCompletions(context, word);
    const triggers = bridge.current.triggerCharacters();
    const charBefore = context.state.sliceDoc(Math.max(0, context.pos - 1), context.pos);
    const isTrigger = triggers.includes(charBefore);

    // Without an explicit request, only offer completions once there is
    // something to complete — otherwise every space fires a round trip.
    if (!context.explicit && !isTrigger && (!word || word.from === word.to)) return null;

    const line = context.state.doc.lineAt(context.pos);
    const response = await bridge.current.completeAt(
      line.number,
      context.pos - line.from + 1,
      isTrigger ? charBefore : undefined,
    );
    if (!response || !response.available || response.items.length === 0) {
      if (!context.explicit) return null;
      return documentCompletions(context, word);
    }

    const total = response.items.length;
    return {
      // A trigger character is not part of the word being replaced.
      from: isTrigger ? context.pos : (word?.from ?? context.pos),
      options: response.items.map((item, index) => toCompletion(item, index, total)),
      // Re-query rather than filter when the server truncated its answer.
      validFor: response.incomplete ? undefined : /^[\w$]*$/,
    };
  };

  // Ctrl-n / Ctrl-p step the completion list, the way a Vim user expects.
  const vimKeys = keymap.of([
    { key: "Ctrl-n", run: (view) => step(view, true) },
    { key: "Ctrl-p", run: (view) => step(view, false) },
  ]);
  const vimCompletionKeys = EditorView.domEventHandlers({
    keydown(event, view) {
      if (event.key.toLowerCase() !== "n" || !event.ctrlKey || event.altKey || event.metaKey) return false;
      event.preventDefault();
      return step(view, true);
    },
  });

  return [vimCompletionKeys, Prec.highest(vimKeys), autocompletion({
    override: [source],
    activateOnTyping: true,
    closeOnBlur: true,
    maxRenderedOptions: 60,
    icons: true,
    defaultKeymap: true,
    tooltipClass: () => "cm-lsp-complete",
  })];
}

function documentCompletions(
  context: CompletionContext,
  word: { readonly from: number; readonly to: number } | null,
): CompletionResult | null {
  const values = new Set<string>();
  const source = context.state.doc.toString();
  for (const match of source.matchAll(/[A-Za-z_$][\w$]*/g)) values.add(match[0]);
  const options = [...values].map((label) => ({ label, type: "text" } satisfies Completion));
  if (options.length === 0) return null;
  return { from: word?.from ?? context.pos, options };
}

function step(view: EditorView, forward: boolean): boolean {
  const status = completionStatus(view.state);
  return status === null ? startCompletion(view) : moveCompletionSelection(forward)(view);
}
