export type HighlightId = number;

export interface Cell {
  readonly text: string;
  readonly hlId: HighlightId;
  readonly width: number;
}

export interface ModeInfo {
  readonly name: string;
  readonly shortName: string;
  readonly cursorShape: "block" | "vertical" | "horizontal";
  readonly cellPercentage: number;
  readonly attrId: HighlightId;
  readonly blinkWait: number;
  readonly blinkOn: number;
  readonly blinkOff: number;
  readonly conceal: string;
  readonly canInsert: boolean;
  readonly canUndo: boolean;
}

export interface CmdlineState {
  readonly content: readonly Cell[];
  readonly position: number;
  readonly firstChar: string;
  readonly prompt: string;
  readonly indent: number;
  readonly specialChar: string | null;
  readonly specialShift: boolean;
  readonly visible: boolean;
  readonly block: readonly (readonly Cell[])[];
}

export type MsgKind =
  | "emsg" | "echo" | "echomsg" | "echoerr" | "echohl" | "warning"
  | "return_prompt" | "search_count" | "quickfix" | "list_cmd" | "autocommand"
  | "fileinfo" | "line" | "question" | "more" | "confirm" | "interrupt"
  | "completion" | "progress" | "shell_err" | "shell_ret" | "history" | "normal";

export interface MessageItem {
  readonly kind: MsgKind | string;
  readonly content: readonly Cell[];
  readonly replaceLast: boolean;
  readonly history: boolean;
}

export interface MessageState {
  readonly items: readonly MessageItem[];
  readonly mode: readonly Cell[];
  readonly command: readonly Cell[];
  readonly ruler: readonly Cell[];
  readonly history: readonly MessageItem[];
  readonly position: number | null;
  readonly scrolled: boolean;
  readonly separator: string;
}

export interface PopupmenuItem {
  readonly word: string;
  readonly abbr: string;
  readonly menu: string;
  readonly info: string;
  readonly kind: string;
  readonly icase: boolean;
  readonly dup: boolean;
  readonly empty: boolean;
}

export interface PopupmenuState {
  readonly items: readonly PopupmenuItem[];
  readonly selected: number;
  readonly row: number;
  readonly col: number;
  readonly visible: boolean;
}

export interface TablineTab {
  readonly tab: string;
  readonly name: string;
  readonly buffer: number;
}

export interface TablineBuffer {
  readonly buffer: number;
  readonly name: string;
}

export interface TablineState {
  readonly current: string;
  readonly tabs: readonly TablineTab[];
  readonly currentTab: string;
  readonly currentBuffer: number;
  readonly buffers: readonly TablineBuffer[];
}

export type PopupmenuItemWire = Record<string, unknown>;
export type MessageHistoryEntry = readonly [MsgKind | string, readonly [number, string, number?][]];
export type UiCell = readonly [hlId: number, text: string, repeat?: number];
export type ModeInfoSet = readonly [enabled: boolean, modes: readonly ModeInfo[]];
export type CmdlineBlock = readonly (readonly UiCell[])[];
