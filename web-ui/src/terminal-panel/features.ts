import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { copyText, readText } from "./clipboard";
import { copyModeStep, selectedRange, type CopyState } from "./copyMode";
import { FONT_CHANGED_EVENT, stepFontSize } from "./fontSize";
import { factsOf, isMac, keyAction, leavesToBrowser, type KeyAction } from "./keys";
import { osc52Text } from "./osc52";

/**
 * Everything that makes the terminal behave like a terminal and not like a text box:
 * copy and paste, select all, font size, OSC 52, wide characters, a right-click menu,
 * middle-click paste, dropped text and a keyboard copy mode.
 *
 * Installed once on a new xterm and removed with it. The panel talks to the result through
 * [`TerminalActions`], which is what the commands and the menu call.
 */

export interface TerminalActions {
  readonly hasSelection: () => boolean;
  readonly copy: () => Promise<boolean>;
  readonly paste: () => Promise<void>;
  readonly selectAll: () => void;
  readonly zoom: (step: -1 | 0 | 1) => void;
  readonly scrollTop: () => void;
  readonly scrollBottom: () => void;
  readonly copyMode: () => void;
  readonly leaveFocus: () => void;
}

export interface FeatureHooks {
  /** Show the right-click menu at a screen position. */
  readonly onMenu: (x: number, y: number) => void;
  /** A short message for the user: "Copied", or why something could not happen. */
  readonly onNotice: (text: string) => void;
  /** Copy mode switched on or off. */
  readonly onCopyMode: (active: boolean) => void;
}

const WHEEL_ZOOM_GAP_MS = 80;

export function installFeatures(
  term: Terminal,
  container: HTMLElement,
  fit: FitAddon,
  hooks: FeatureHooks,
): { actions: TerminalActions; dispose: () => void } {
  const mac = isMac();
  const cleanups: Array<() => void> = [];
  let copyState: CopyState | null = null;

  const unicode = new Unicode11Addon();
  term.loadAddon(unicode);
  term.unicode.activeVersion = "11";

  const copy = async (): Promise<boolean> => {
    const text = term.getSelection();
    if (!text) return false;
    const done = await copyText(text);
    hooks.onNotice(done ? "Copied" : "Could not copy to the clipboard");
    return done;
  };

  const paste = async (): Promise<void> => {
    const text = await readText();
    if (text === null) {
      hooks.onNotice(`The browser blocked reading the clipboard. Press ${mac ? "Cmd+V" : "Ctrl+Shift+V"}.`);
      return;
    }
    if (text) term.paste(text);
    term.focus();
  };

  const geometry = () => ({ rows: term.buffer.active.length, page: term.rows });

  const showCopyCursor = (state: CopyState) => {
    const [first, last] = selectedRange(state);
    term.selectLines(first, last);
    const top = term.buffer.active.viewportY;
    if (state.cursor < top) term.scrollToLine(state.cursor);
    else if (state.cursor > top + term.rows - 1) term.scrollToLine(state.cursor - term.rows + 1);
  };

  const leaveCopyMode = () => {
    copyState = null;
    term.clearSelection();
    hooks.onCopyMode(false);
  };

  const enterCopyMode = () => {
    const buffer = term.buffer.active;
    copyState = { cursor: buffer.viewportY + term.rows - 1, anchor: null };
    hooks.onCopyMode(true);
    showCopyCursor(copyState);
  };

  const copyModeKey = (event: KeyboardEvent): boolean => {
    if (!copyState || event.type !== "keydown") return copyState !== null;
    if (event.metaKey || event.altKey) return true;
    const step = copyModeStep(copyState, event.key, event.ctrlKey, geometry());
    event.preventDefault();
    copyState = step.state;
    if (step.effect === "copy") {
      void copy().then(leaveCopyMode);
    } else if (step.effect === "exit") {
      leaveCopyMode();
    } else {
      showCopyCursor(step.state);
    }
    return true;
  };

  const zoom = (step: -1 | 0 | 1) => {
    stepFontSize(step);
  };

  const run: Record<KeyAction, () => void> = {
    copy: () => void copy(),
    paste: () => void 0,
    selectAll: () => term.selectAll(),
    zoomIn: () => zoom(1),
    zoomOut: () => zoom(-1),
    zoomReset: () => zoom(0),
    scrollTop: () => term.scrollToTop(),
    scrollBottom: () => term.scrollToBottom(),
  };

  term.attachCustomKeyEventHandler((event) => {
    if (copyModeKey(event)) return false;
    const action = keyAction(factsOf(event), mac, term.hasSelection());
    if (!action) return true;
    if (event.type !== "keydown") return false;
    // A paste is finished by the browser's own paste event, which carries the clipboard
    // without asking permission. Everything else is done here and kept from the page.
    if (!leavesToBrowser(action)) {
      event.preventDefault();
      run[action]();
    }
    return false;
  });

  // A program copying through the terminal (tmux, Neovim, over SSH).
  const osc = term.parser.registerOscHandler(52, (payload) => {
    const text = osc52Text(payload);
    if (text) void copyText(text);
    return true;
  });
  cleanups.push(() => osc.dispose());

  const onFont = (event: Event) => {
    const size = (event as CustomEvent<number>).detail;
    if (typeof size !== "number" || term.options.fontSize === size) return;
    term.options.fontSize = size;
    fit.fit();
  };
  window.addEventListener(FONT_CHANGED_EVENT, onFont);
  cleanups.push(() => window.removeEventListener(FONT_CHANGED_EVENT, onFont));

  // A program that tracks the mouse (vim, tmux) gets the right click; Shift takes it back.
  const onContextMenu = (event: MouseEvent) => {
    const appOwnsMouse = term.modes.mouseTrackingMode !== "none" && !event.shiftKey;
    if (appOwnsMouse) return;
    event.preventDefault();
    hooks.onMenu(event.clientX, event.clientY);
  };
  container.addEventListener("contextmenu", onContextMenu);
  cleanups.push(() => container.removeEventListener("contextmenu", onContextMenu));

  const onAuxClick = (event: MouseEvent) => {
    if (event.button !== 1 || term.modes.mouseTrackingMode !== "none") return;
    event.preventDefault();
    void paste();
  };
  container.addEventListener("auxclick", onAuxClick);
  cleanups.push(() => container.removeEventListener("auxclick", onAuxClick));

  let lastWheelZoom = 0;
  const onWheel = (event: WheelEvent) => {
    if (!(event.ctrlKey || event.metaKey)) return;
    event.preventDefault();
    const now = performance.now();
    if (now - lastWheelZoom < WHEEL_ZOOM_GAP_MS) return;
    lastWheelZoom = now;
    zoom(event.deltaY < 0 ? 1 : -1);
  };
  container.addEventListener("wheel", onWheel, { passive: false });
  cleanups.push(() => container.removeEventListener("wheel", onWheel));

  const onDragOver = (event: DragEvent) => {
    if (event.dataTransfer?.types.includes("text/plain")) event.preventDefault();
  };
  const onDrop = (event: DragEvent) => {
    const text = event.dataTransfer?.getData("text/plain");
    if (!text) return;
    event.preventDefault();
    term.paste(text);
    term.focus();
  };
  container.addEventListener("dragover", onDragOver);
  container.addEventListener("drop", onDrop);
  cleanups.push(() => {
    container.removeEventListener("dragover", onDragOver);
    container.removeEventListener("drop", onDrop);
  });

  const actions: TerminalActions = {
    hasSelection: () => term.hasSelection(),
    copy,
    paste,
    selectAll: () => term.selectAll(),
    zoom,
    scrollTop: () => term.scrollToTop(),
    scrollBottom: () => term.scrollToBottom(),
    copyMode: enterCopyMode,
    leaveFocus: () => {
      term.blur();
      container.closest<HTMLElement>("[data-pane-id]")?.focus();
    },
  };
  return { actions, dispose: () => cleanups.forEach((undo) => undo()) };
}
