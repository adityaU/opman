import { forEachDiagnostic } from "@codemirror/lint";
import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

const tooltips = new WeakMap<EditorView, HTMLElement>();

/** Keep diagnostic hover visible even while the async LSP poll refreshes state. */
export function diagnosticHoverExtension(): Extension {
  return EditorView.domEventHandlers({
    mouseover(event, view) {
      const target = event.target instanceof Element
        ? event.target.closest<HTMLElement>(".cm-lintRange")
        : null;
      if (!target) return false;
      const point = view.posAtCoords({ x: event.clientX, y: event.clientY });
      if (point === null) return false;
      const message = diagnosticMessage(view, point);
      if (!message) return false;
      tooltips.get(view)?.remove();
      const tooltip = document.createElement("div");
      tooltip.className = "cm-tooltip cm-tooltip-lint cm-lsp-diagnostic";
      tooltip.textContent = message;
      const rect = target.getBoundingClientRect();
      tooltip.style.position = "fixed";
      tooltip.style.left = `${rect.left}px`;
      tooltip.style.top = `${Math.max(4, rect.bottom + 6)}px`;
      document.body.append(tooltip);
      tooltips.set(view, tooltip);
      return false;
    },
    mouseout(event, view) {
      if (event.relatedTarget instanceof Node && view.dom.contains(event.relatedTarget)) return false;
      tooltips.get(view)?.remove();
      tooltips.delete(view);
      return false;
    },
  });
}

function diagnosticMessage(view: EditorView, pos: number): string | null {
  let message: string | null = null;
  forEachDiagnostic(view.state, (diagnostic, from, to) => {
    if (message || pos < from || pos > to) return;
    message = diagnostic.message;
  });
  return message;
}
