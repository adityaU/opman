import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { describe, expect, it, vi, afterEach } from "vitest";
import {
  byteToCodePoint, byteToUtf16, codePointToByte, utf16ToByte,
} from "../edit/columns";
import { emptyOverlays } from "../edit/overlays";
import { bindingSnapshotEffect, bindingStateField, decorationsFor, type BindingSnapshot } from "../edit/decorations";
import { NVIM_CAPTURE_ATTRIBUTE } from "../capture";
import { NvimEditBinding } from "../edit/binding";
import { routeNvimKey } from "../edit/keyRouting";
import { lineChangeToSpec } from "../edit/sync";

describe("Neovim edit sync", () => {
  it("maps inserts, deletes, replacements, multiline changes, and end inserts", () => {
    const doc = EditorState.create({ doc: "one\ntwo\nthree" }).doc;
    expect(lineChangeToSpec(doc, { first_line: 1, last_line: 2, lines: ["TWO"] })).toEqual({ from: 4, to: 8, insert: "TWO\n" });
    expect(lineChangeToSpec(doc, { first_line: 0, last_line: 1, lines: [] })).toEqual({ from: 0, to: 4, insert: "" });
    expect(lineChangeToSpec(doc, { first_line: 2, last_line: 3, lines: [] })).toEqual({ from: 7, to: 13, insert: "" });
    expect(lineChangeToSpec(doc, { first_line: 1, last_line: 2, lines: ["two-a", "two-b"] })).toEqual({ from: 4, to: 8, insert: "two-a\ntwo-b\n" });
    expect(lineChangeToSpec(doc, { first_line: 3, last_line: 3, lines: ["four"] })).toEqual({ from: 13, to: 13, insert: "\nfour" });
  });

  it("round-trips byte, code-point, and UTF-16 columns for Unicode and tabs", () => {
    const value = "😀界e\u{301}\tx";
    const boundaries = [0, 4, 7, 8, 10, 11, 12];
    const utf16 = [0, 2, 3, 4, 5, 6, 7];
    boundaries.forEach((byte, index) => {
      expect(byteToUtf16(value, byte)).toBe(utf16[index]);
      expect(utf16ToByte(value, utf16[index])).toBe(byte);
    });
    expect(byteToCodePoint(value, 10)).toBe(4);
    expect(codePointToByte(value, 4)).toBe(10);
    expect(utf16ToByte(value, 1)).toBe(-1);
  });

  it("routes normal keys to Neovim and leaves insert typing local", () => {
    const normal = new KeyboardEvent("keydown", { key: "h", code: "KeyH" });
    const insert = new KeyboardEvent("keydown", { key: "a", code: "KeyA" });
    const completion = new KeyboardEvent("keydown", { key: "n", code: "KeyN", ctrlKey: true });
    const escape = new KeyboardEvent("keydown", { key: "Escape", code: "Escape" });
    const input = vi.fn();
    expect(routeNvimKey(normal, "normal", input, vi.fn())).toBe(true);
    expect(input).toHaveBeenCalledWith("h");
    expect(routeNvimKey(insert, "insert", input, vi.fn())).toBe(false);
    expect(routeNvimKey(completion, "insert", input, vi.fn())).toBe(true);
    expect(routeNvimKey(escape, "insert", input, vi.fn())).toBe(true);
    expect(input).toHaveBeenLastCalledWith("<Esc>");
  });

  it("renders normal, insert, replace, and visual-block state as CM decorations", () => {
    const state = EditorState.create({ doc: "abcd\nefgh\nijkl" });
    const base: BindingSnapshot = {
      mode: "n", modeShort: "normal", cursor: { line: 0, column: 1 }, visual: null,
      changedtick: 1, connection: { status: "attached" }, message: null, overlays: emptyOverlays,
    };
    expect(decorationClasses(decorationsFor(state, base))).toContain("cm-nvim-cursor-block");
    expect(decorationClasses(decorationForMode(state, { ...base, modeShort: "insert" }))).toContain("cm-nvim-cursor-bar");
    expect(decorationClasses(decorationForMode(state, { ...base, modeShort: "replace" }))).toContain("cm-nvim-cursor-underline");
    const block = decorationForMode(state, {
      ...base,
      modeShort: "visual_block",
      visual: { start: { line: 0, column: 1 }, end: { line: 2, column: 2 } },
    });
    expect(decorationClasses(block).filter((name) => name === "cm-nvim-visual-selection")).toHaveLength(3);
  });

  it("publishes the capture claim only while attached", () => {
    // Dispatching snapshots directly isolates the state-field provider that
    // publishes the DOM claim.
    const parent = document.createElement("div");
    const view = new EditorView({
      state: EditorState.create({ doc: "abc", extensions: [bindingStateField] }),
      parent,
    });
    expect(view.dom.hasAttribute(NVIM_CAPTURE_ATTRIBUTE)).toBe(false);

    const attached: BindingSnapshot = {
      mode: "n", modeShort: "normal", cursor: null, visual: null,
      changedtick: 1, connection: { status: "attached" }, message: null, overlays: emptyOverlays,
    };
    view.dispatch({ effects: bindingSnapshotEffect.of(attached) });
    expect(view.dom.getAttribute(NVIM_CAPTURE_ATTRIBUTE)).toBe("true");

    view.dispatch({ effects: bindingSnapshotEffect.of({
      ...attached, connection: { status: "failed", reason: "offline" },
    }) });
    expect(view.dom.hasAttribute(NVIM_CAPTURE_ATTRIBUTE)).toBe(false);

    view.dispatch({ effects: bindingSnapshotEffect.of({
      ...attached, connection: { status: "idle", reason: "disabled" },
    }) });
    expect(view.dom.hasAttribute(NVIM_CAPTURE_ATTRIBUTE)).toBe(false);
    view.destroy();
  });
});

describe("echo suppression", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("does not dispatch an echoed BufferChanged a second time", () => {
    class FakeSocket {
      static readonly OPEN = 1;
      static last: FakeSocket | null = null;
      readonly readyState = FakeSocket.OPEN;
      readonly sent: string[] = [];
      onopen: (() => void) | null = null;
      onmessage: ((event: MessageEvent<string>) => void) | null = null;
      onerror: (() => void) | null = null;
      onclose: (() => void) | null = null;
      send(value: string): void { this.sent.push(value); }
      close(): void { this.onclose?.(); }
    }
    vi.stubGlobal("WebSocket", class extends FakeSocket {
      constructor() { super(); FakeSocket.last = this; }
    });
    const binding = new NvimEditBinding();
    const parent = document.createElement("div");
    const view = new EditorView({ state: EditorState.create({ doc: "abc", extensions: [binding.extension] }), parent });
    binding.setOptions({ enabled: true, path: "/repo/file.ts", sessionId: "session", idleReason: "disabled" });
    const socket = FakeSocket.last;
    expect(socket).not.toBeNull();
    socket?.onopen?.();
    view.dispatch({ changes: { from: 3, insert: "!" } });
    const edit = JSON.parse(socket?.sent.at(-1) ?? "{}") as { edit_id?: string; changedtick?: number };
    const dispatch = vi.spyOn(view, "dispatch");
    socket?.onmessage?.(new MessageEvent("message", {
      data: JSON.stringify({
        type: "buffer_changed", buffer: 1, changedtick: (edit.changedtick ?? 0) + 1,
        first_line: 0, last_line: 1, new_last_line: 1, lines: ["abc!"], origin: edit.edit_id,
      }),
    }));
    expect(view.state.doc.toString()).toBe("abc!");
    expect(dispatch).not.toHaveBeenCalled();
    view.destroy();
    binding.dispose();
  });

  it("re-attaches on a stale changedtick resync request", () => {
    class FakeSocket {
      static readonly OPEN = 1;
      static last: FakeSocket | null = null;
      readonly readyState = FakeSocket.OPEN;
      readonly sent: string[] = [];
      onopen: (() => void) | null = null;
      onmessage: ((event: MessageEvent<string>) => void) | null = null;
      onerror: (() => void) | null = null;
      onclose: (() => void) | null = null;
      send(value: string): void { this.sent.push(value); }
      close(): void { this.onclose?.(); }
    }
    vi.stubGlobal("WebSocket", class extends FakeSocket {
      constructor() { super(); FakeSocket.last = this; }
    });
    const binding = new NvimEditBinding();
    const view = new EditorView({ state: EditorState.create({ doc: "abc", extensions: [binding.extension] }), parent: document.createElement("div") });
    binding.setOptions({ enabled: true, path: "/repo/file.ts", sessionId: "session", idleReason: "disabled" });
    const socket = FakeSocket.last;
    socket?.onopen?.();
    const before = socket?.sent.length ?? 0;
    socket?.onmessage?.(new MessageEvent("message", { data: JSON.stringify({ type: "resync_required", changedtick: 9, reason: "stale" }) }));
    expect(socket?.sent.length).toBe(before + 1);
    expect(JSON.parse(socket?.sent.at(-1) ?? "{}")).toEqual({ type: "attach", path: "/repo/file.ts" });
    view.destroy();
    binding.dispose();
  });
});

function decorationForMode(state: EditorState, snapshot: BindingSnapshot) {
  return decorationsFor(state, snapshot);
}

function decorationClasses(set: ReturnType<typeof decorationsFor>): string[] {
  const classes: string[] = [];
  set.between(0, 1000, (_from, _to, value) => {
    const name = value.spec.class;
    if (typeof name === "string") classes.push(...name.split(" "));
  });
  return classes;
}
