import type { ClientMsg } from "../wire/control";

export type MouseButton = "left" | "middle" | "right" | "x1" | "x2" | "wheel";
export type MouseAction = "press" | "drag" | "release" | "up" | "down" | "left" | "right";

export interface MouseLayout {
  readonly grid: number;
  readonly left: number;
  readonly top: number;
  readonly cellWidth: number;
  readonly cellHeight: number;
  readonly width?: number;
  readonly height?: number;
}

export interface MouseInputOptions {
  readonly layout: () => MouseLayout;
  readonly layoutForPoint?: (clientX: number, clientY: number) => MouseLayout | null;
  readonly send: (message: ClientMsg) => void;
  readonly lineHeight?: number;
  readonly pageLines?: number;
  readonly requestFrame?: (callback: FrameRequestCallback) => number;
  readonly cancelFrame?: (handle: number) => void;
}

export interface MouseHandlers {
  readonly onPointerDown: (event: PointerEvent) => void;
  readonly onPointerMove: (event: PointerEvent) => void;
  readonly onPointerUp: (event: PointerEvent) => void;
  readonly onWheel: (event: WheelEvent) => void;
  readonly flushDrag: () => void;
  readonly dispose: () => void;
}

function buttonFromPointer(button: number): MouseButton | null {
  if (button === 0) return "left";
  if (button === 1) return "middle";
  if (button === 2) return "right";
  if (button === 3) return "x1";
  if (button === 4) return "x2";
  return null;
}

function buttonFromButtons(buttons: number): MouseButton | null {
  if ((buttons & 1) !== 0) return "left";
  if ((buttons & 4) !== 0) return "middle";
  if ((buttons & 2) !== 0) return "right";
  if ((buttons & 8) !== 0) return "x1";
  if ((buttons & 16) !== 0) return "x2";
  return null;
}

/** Neovim uses M for a mouse Meta modifier; key notation uses D separately. */
export function encodeMouseModifier(event: Pick<PointerEvent, "shiftKey" | "ctrlKey" | "altKey" | "metaKey">): string {
  let modifier = "";
  if (event.shiftKey) modifier += "S";
  if (event.ctrlKey) modifier += "C";
  if (event.altKey) modifier += "A";
  if (event.metaKey) modifier += "M";
  return modifier;
}

function cellCoordinate(value: number, origin: number, cell: number, limit: number | undefined): number {
  const coordinate = Math.max(0, Math.floor((value - origin) / cell));
  return limit === undefined ? coordinate : Math.min(coordinate, Math.max(0, limit - 1));
}

export function pointerToMouseMessage(
  event: Pick<PointerEvent, "clientX" | "clientY" | "shiftKey" | "ctrlKey" | "altKey" | "metaKey">,
  layout: MouseLayout,
  action: MouseAction,
  button: MouseButton,
): ClientMsg {
  return {
    type: "input_mouse",
    button,
    action,
    modifier: encodeMouseModifier(event),
    grid: layout.grid,
    row: cellCoordinate(event.clientY, layout.top, layout.cellHeight, layout.height),
    col: cellCoordinate(event.clientX, layout.left, layout.cellWidth, layout.width),
  };
}

function wheelLines(delta: number, mode: number, lineHeight: number, pageLines: number): number {
  if (mode === WheelEvent.DOM_DELTA_LINE || mode === 1) return delta;
  if (mode === WheelEvent.DOM_DELTA_PAGE || mode === 2) return delta * pageLines;
  return delta / lineHeight;
}

function wheelMessages(
  event: Pick<WheelEvent, "clientX" | "clientY" | "deltaX" | "deltaY" | "deltaMode" | "shiftKey" | "ctrlKey" | "altKey" | "metaKey">,
  layout: MouseLayout,
  lineHeight: number,
  pageLines: number,
  accumulated: { x: number; y: number },
): ClientMsg[] {
  accumulated.x += wheelLines(event.deltaX, event.deltaMode, lineHeight, pageLines);
  accumulated.y += wheelLines(event.deltaY, event.deltaMode, lineHeight, pageLines);
  const messages: ClientMsg[] = [];
  const makeWheel = (action: MouseAction): ClientMsg => pointerToMouseMessage(
    event,
    layout,
    action,
    "wheel",
  );
  const vertical = Math.trunc(accumulated.y);
  const horizontal = Math.trunc(accumulated.x);
  if (vertical !== 0) {
    for (let index = 0; index < Math.abs(vertical); index += 1) {
      messages.push(makeWheel(vertical < 0 ? "up" : "down"));
    }
    accumulated.y -= vertical;
  }
  if (horizontal !== 0) {
    for (let index = 0; index < Math.abs(horizontal); index += 1) {
      messages.push(makeWheel(horizontal < 0 ? "left" : "right"));
    }
    accumulated.x -= horizontal;
  }
  return messages;
}

/** Build pointer/wheel handlers. Drag events are coalesced to one message per animation frame. */
export function createMouseHandlers(options: MouseInputOptions): MouseHandlers {
  const requestFrame = options.requestFrame ?? ((callback: FrameRequestCallback) => requestAnimationFrame(callback));
  const cancelFrame = options.cancelFrame ?? ((handle: number) => cancelAnimationFrame(handle));
  const accumulated = { x: 0, y: 0 };
  let pending: PointerEvent | null = null;
  let frame: number | null = null;
  const layoutForEvent = (event: Pick<PointerEvent, "clientX" | "clientY">): MouseLayout => options.layoutForPoint?.(event.clientX, event.clientY) ?? options.layout();

  const flushDrag = (): void => {
    frame = null;
    const event = pending;
    pending = null;
    if (event === null) return;
    const button = buttonFromButtons(event.buttons);
    if (button !== null) options.send(pointerToMouseMessage(event, layoutForEvent(event), "drag", button));
  };

  const onPointerDown = (event: PointerEvent): void => {
    const button = buttonFromPointer(event.button);
    if (button === null) return;
    event.preventDefault();
    options.send(pointerToMouseMessage(event, layoutForEvent(event), "press", button));
  };

  const onPointerMove = (event: PointerEvent): void => {
    if (buttonFromButtons(event.buttons) === null) return;
    event.preventDefault();
    pending = event;
    if (frame === null) frame = requestFrame(flushDrag);
  };

  const onPointerUp = (event: PointerEvent): void => {
    const button = buttonFromPointer(event.button);
    if (button === null) return;
    event.preventDefault();
    flushDrag();
    options.send(pointerToMouseMessage(event, layoutForEvent(event), "release", button));
  };

  const onWheel = (event: WheelEvent): void => {
    const messages = wheelMessages(
      event,
      layoutForEvent(event),
      options.lineHeight ?? 16,
      options.pageLines ?? 3,
      accumulated,
    );
    if (messages.length === 0) return;
    event.preventDefault();
    for (const message of messages) options.send(message);
  };

  return {
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onWheel,
    flushDrag,
    dispose: () => {
      pending = null;
      if (frame !== null) cancelFrame(frame);
      frame = null;
    },
  };
}

export const createMouseInput = createMouseHandlers;
