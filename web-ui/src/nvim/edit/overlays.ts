/** Neovim surfaces that live beside the document: command line, messages, tabs. */

import type { ControlMsg, NvimLayout } from "./wire";

export interface CmdlineOverlay {
  readonly visible: boolean;
  readonly firstChar: string;
  readonly content: string;
  readonly position: number;
}

export interface NvimMessage {
  readonly kind: string;
  readonly text: string;
}

export interface OverlayState {
  readonly cmdline: CmdlineOverlay;
  readonly messages: readonly NvimMessage[];
  readonly search: string | null;
  readonly layout: NvimLayout;
}

const MAX_MESSAGES = 20;

export const emptyCmdline: CmdlineOverlay = { visible: false, firstChar: "", content: "", position: 0 };

export const emptyLayout: NvimLayout = { tabpages: 1, windows: 1, buffers: [] };

export const emptyOverlays: OverlayState = {
  cmdline: emptyCmdline,
  messages: [],
  search: null,
  layout: emptyLayout,
};

type OverlayMsg = Extract<ControlMsg, { type: "cmdline" | "search" | "layout" | "message" }>;

/** Fold one control message into the overlay state, or return it unchanged. */
export function reduceOverlays(state: OverlayState, message: OverlayMsg): OverlayState {
  switch (message.type) {
    case "cmdline":
      return {
        ...state,
        cmdline: {
          visible: message.visible,
          firstChar: message.first_char,
          content: message.content,
          position: message.position,
        },
      };
    case "search":
      return { ...state, search: message.pattern };
    case "layout":
      return { ...state, layout: message.layout };
    case "message":
      return { ...state, messages: appendMessage(state.messages, message.kind, message.text) };
  }
}

function appendMessage(
  messages: readonly NvimMessage[],
  kind: string,
  text: string,
): readonly NvimMessage[] {
  if (text.length === 0) return messages;
  const next = [...messages, { kind, text }];
  return next.length > MAX_MESSAGES ? next.slice(next.length - MAX_MESSAGES) : next;
}

/** What to highlight: the search line Neovim is drawing, else its register. */
export function searchPattern(state: OverlayState): string | null {
  const { cmdline, search } = state;
  if (cmdline.visible) return cmdline.firstChar === "/" || cmdline.firstChar === "?" ? cmdline.content : null;
  return search;
}

export function messageTone(kind: string): "error" | "warning" | "info" {
  if (kind === "emsg" || kind === "echoerr" || kind === "shell_err") return "error";
  if (kind === "wmsg" || kind === "warning" || kind === "question" || kind === "confirm") return "warning";
  return "info";
}

/** The `-- INSERT --` line Neovim shows for modes that accept typed text. */
export function showmodeLabel(mode: string): string | null {
  switch (mode) {
    case "insert": return "-- INSERT --";
    case "replace": return "-- REPLACE --";
    case "visual": return "-- VISUAL --";
    case "visual_line": return "-- VISUAL LINE --";
    case "visual_block": return "-- VISUAL BLOCK --";
    default: return null;
  }
}
