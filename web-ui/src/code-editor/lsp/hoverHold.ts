import { StateEffect, StateField } from "@codemirror/state";
import type { Extension } from "@codemirror/state";
import { EditorView, showTooltip, ViewPlugin, type PluginValue, type Tooltip } from "@codemirror/view";

/**
 * A hover tooltip that lingers.
 *
 * CodeMirror's own `hoverTooltip` drops the tooltip the moment the pointer
 * leaves the word, which is right for a one-line type hint and wrong for a card
 * with buttons in it: the reader moves toward the action and the thing they were
 * reaching for disappears. There is no delay option to set, so the hover
 * lifecycle is managed here instead.
 *
 * Held open for `HOLD_MS` after the pointer leaves, cancelled early only by a
 * hover somewhere else — which is the one case where keeping the old card would
 * be showing the wrong symbol.
 */

const HOLD_MS = 5_000;

/** How long to wait, once the pointer settles, before asking the server. */
const SETTLE_MS = 80;

export interface HoverSource {
  /** Resolve the tooltip for a position, or null when there is nothing to say. */
  (view: EditorView, pos: number): Promise<Tooltip | null>;
}

const setHover = StateEffect.define<Tooltip | null>();

const hoverField = StateField.define<Tooltip | null>({
  create: () => null,
  update(value, transaction) {
    for (const effect of transaction.effects) if (effect.is(setHover)) return effect.value;
    // An edit invalidates the position the card describes, so it goes rather
    // than hanging over text that has moved.
    return transaction.docChanged ? null : value;
  },
  provide: (field) => showTooltip.from(field),
});

class HoverHold implements PluginValue {
  private settle: ReturnType<typeof setTimeout> | undefined;
  private hide: ReturnType<typeof setTimeout> | undefined;
  /** The position the visible card describes, so a re-hover there is a no-op. */
  private shown: number | null = null;
  private generation = 0;

  constructor(
    private readonly view: EditorView,
    private readonly source: HoverSource,
  ) {
    this.onMove = this.onMove.bind(this);
    this.onLeave = this.onLeave.bind(this);
    view.dom.addEventListener("mousemove", this.onMove);
    view.dom.addEventListener("mouseleave", this.onLeave);
  }

  destroy(): void {
    this.view.dom.removeEventListener("mousemove", this.onMove);
    this.view.dom.removeEventListener("mouseleave", this.onLeave);
    this.clearTimers();
  }

  private clearTimers(): void {
    if (this.settle !== undefined) clearTimeout(this.settle);
    if (this.hide !== undefined) clearTimeout(this.hide);
    this.settle = undefined;
    this.hide = undefined;
  }

  private show(tooltip: Tooltip | null, pos: number | null): void {
    this.shown = tooltip ? pos : null;
    this.view.dispatch({ effects: setHover.of(tooltip) });
  }

  /** Start the grace period. Called on every move away from the shown word. */
  private beginHold(): void {
    if (this.shown === null || this.hide !== undefined) return;
    this.hide = setTimeout(() => {
      this.hide = undefined;
      this.show(null, null);
    }, HOLD_MS);
  }

  private cancelHold(): void {
    if (this.hide === undefined) return;
    clearTimeout(this.hide);
    this.hide = undefined;
  }

  private onMove(event: MouseEvent): void {
    // Inside the card itself: the reader is using it, so nothing expires.
    if (event.target instanceof Node && this.view.dom.contains(event.target)) {
      const tooltip = (event.target as Element).closest?.(".cm-tooltip");
      if (tooltip) {
        this.cancelHold();
        return;
      }
    }

    const pos = this.view.posAtCoords({ x: event.clientX, y: event.clientY });
    if (pos === null) {
      this.beginHold();
      return;
    }
    if (this.shown !== null && sameWord(this.view, pos, this.shown)) {
      this.cancelHold();
      return;
    }

    if (this.settle !== undefined) clearTimeout(this.settle);
    this.settle = setTimeout(() => {
      this.settle = undefined;
      void this.query(pos);
    }, SETTLE_MS);
    this.beginHold();
  }

  private async query(pos: number): Promise<void> {
    const generation = ++this.generation;
    const tooltip = await this.source(this.view, pos);
    if (generation !== this.generation) return;
    if (!tooltip) {
      // Nothing here. The card already up keeps its grace period rather than
      // vanishing because the pointer crossed a comma.
      return;
    }
    this.cancelHold();
    this.show(tooltip, pos);
  }

  private onLeave(): void {
    if (this.settle !== undefined) clearTimeout(this.settle);
    this.settle = undefined;
    this.beginHold();
  }
}

/** Whether two positions fall inside the same word, so a jitter is not a move. */
function sameWord(view: EditorView, left: number, right: number): boolean {
  if (left === right) return true;
  const word = view.state.wordAt(right);
  return word ? left >= word.from && left <= word.to : false;
}

/**
 * Tooltips are rendered in the editor's own DOM so the pointer can enter them
 * without the browser treating it as leaving the editor.
 */
export function holdingHoverTooltip(source: HoverSource): Extension {
  return [
    hoverField,
    ViewPlugin.define((view) => new HoverHold(view, source)),
    tooltipsStayInside,
  ];
}

const tooltipsStayInside = EditorView.baseTheme({
  ".cm-tooltip.cm-tooltip-hover": { pointerEvents: "auto" },
});
