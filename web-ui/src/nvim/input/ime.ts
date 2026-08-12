import type { ClientMsg } from "../wire/control";
import { pasteMessage } from "./paste";

export interface ImeCursor {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

export interface ImeInputOptions {
  readonly container: HTMLElement;
  readonly send: (message: ClientMsg) => void;
  readonly cursor?: ImeCursor;
}

export interface ImeInput {
  readonly element: HTMLDivElement;
  readonly updateCursor: (cursor: ImeCursor) => void;
  readonly focus: () => void;
  readonly blur: () => void;
  readonly destroy: () => void;
}

function clear(element: HTMLDivElement): void {
  element.textContent = "";
}

function position(element: HTMLDivElement, cursor: ImeCursor): void {
  element.style.left = `${cursor.left}px`;
  element.style.top = `${cursor.top}px`;
  element.style.width = `${Math.max(1, cursor.width)}px`;
  element.style.height = `${Math.max(1, cursor.height)}px`;
}

/** Install a non-displayed-but-focusable contenteditable for browser IME composition. */
export function createImeInput(options: ImeInputOptions): ImeInput {
  const element = document.createElement("div");
  element.contentEditable = "true";
  element.spellcheck = false;
  element.setAttribute("aria-hidden", "true");
  element.setAttribute("role", "presentation");
  element.tabIndex = -1;
  element.style.position = "absolute";
  element.style.zIndex = "-1";
  element.style.opacity = "0";
  element.style.pointerEvents = "none";
  element.style.whiteSpace = "pre";
  element.style.overflow = "hidden";
  if (options.cursor) position(element, options.cursor);
  options.container.appendChild(element);

  let composing = false;
  const onCompositionStart = (): void => {
    composing = true;
    clear(element);
  };
  const onCompositionUpdate = (): void => {
  };
  const onCompositionEnd = (event: CompositionEvent): void => {
    composing = false;
    const data = event.data || element.textContent || "";
    clear(element);
    if (data.length > 0) options.send(pasteMessage(data));
  };
  const onInput = (): void => {
    if (composing) return;
    const data = element.textContent || "";
    clear(element);
    if (data.length > 0) options.send(pasteMessage(data));
  };

  element.addEventListener("compositionstart", onCompositionStart);
  element.addEventListener("compositionupdate", onCompositionUpdate);
  element.addEventListener("compositionend", onCompositionEnd);
  element.addEventListener("input", onInput);

  return {
    element,
    updateCursor: (cursor) => position(element, cursor),
    focus: () => {
      element.focus({ preventScroll: true });
    },
    blur: () => element.blur(),
    destroy: () => {
      element.removeEventListener("compositionstart", onCompositionStart);
      element.removeEventListener("compositionupdate", onCompositionUpdate);
      element.removeEventListener("compositionend", onCompositionEnd);
      element.removeEventListener("input", onInput);
      element.remove();
    },
  };
}

export const createImeController = createImeInput;
