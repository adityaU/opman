import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NvimEditBinding } from "./binding";

class FakeSocket {
  static readonly OPEN = 1;
  static last: FakeSocket | null = null;
  readonly readyState = FakeSocket.OPEN;
  readonly sent: string[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: MessageEvent<string>) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;
  constructor() { FakeSocket.last = this; }
  send(value: string): void { this.sent.push(value); }
  close(): void { this.onclose?.(); }
}

describe("NvimEditBinding input ordering", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("advances the queue only for an explicit input acknowledgment", () => {
    vi.stubGlobal("WebSocket", FakeSocket);
    const binding = new NvimEditBinding();
    const parent = document.createElement("div");
    document.body.append(parent);
    const view = new EditorView({
      state: EditorState.create({ doc: "hello", extensions: [binding.extension] }),
      parent,
    });
    binding.setOptions({ enabled: true, path: "/repo/file.txt", sessionId: "session", idleReason: "disabled" });
    const socket = FakeSocket.last;
    socket?.onopen?.();
    socket?.onmessage?.(new MessageEvent("message", {
      data: JSON.stringify({ type: "attached", buffer: 1, path: "/repo/file.txt", changedtick: 1, lines: ["hello"] }),
    }));
    const state = { type: "state", changedtick: 1, cursor: { line: 0, column: 0 }, mode: "n", mode_short: "normal", visual: null };
    socket?.onmessage?.(new MessageEvent("message", { data: JSON.stringify(state) }));

    view.dom.dispatchEvent(new KeyboardEvent("keydown", { key: "d", code: "KeyD", bubbles: true }));
    view.dom.dispatchEvent(new KeyboardEvent("keydown", { key: "w", code: "KeyW", bubbles: true }));
    expect(socket?.sent.map((value) => JSON.parse(value).keys).filter(Boolean)).toEqual(["d"]);

    socket?.onmessage?.(new MessageEvent("message", {
      data: JSON.stringify({ ...state, mode: "no", mode_short: "operator_pending" }),
    }));
    expect(socket?.sent.map((value) => JSON.parse(value).keys).filter(Boolean)).toEqual(["d"]);

    socket?.onmessage?.(new MessageEvent("message", { data: JSON.stringify({ type: "input_ack" }) }));
    expect(socket?.sent.map((value) => JSON.parse(value).keys).filter(Boolean)).toEqual(["d", "w"]);

    view.destroy();
    parent.remove();
    binding.dispose();
  });

  it.each([
    ["long motions", ["g", "g", "G", "g", "g", "j", "j", "k"]],
    ["word, line, and text-object operators", ["d", "w", "y", "y", "d", "i", "w"]],
    ["named-register paste", ["\"", "a", "p"]],
  ])("preserves every key in %s", (_name, keys) => {
    vi.stubGlobal("WebSocket", FakeSocket);
    const binding = new NvimEditBinding();
    const parent = document.createElement("div");
    document.body.append(parent);
    const view = new EditorView({
      state: EditorState.create({ doc: "hello", extensions: [binding.extension] }),
      parent,
    });
    binding.setOptions({ enabled: true, path: "/repo/file.txt", sessionId: "session", idleReason: "disabled" });
    const socket = FakeSocket.last;
    socket?.onopen?.();
    socket?.onmessage?.(new MessageEvent("message", {
      data: JSON.stringify({ type: "attached", buffer: 1, path: "/repo/file.txt", changedtick: 1, lines: ["hello"] }),
    }));
    socket?.onmessage?.(new MessageEvent("message", {
      data: JSON.stringify({ type: "state", changedtick: 1, cursor: { line: 0, column: 0 }, mode: "n", mode_short: "normal", visual: null }),
    }));

    for (const key of keys) {
      view.dom.dispatchEvent(new KeyboardEvent("keydown", {
        key,
        code: key.toLowerCase() >= "a" && key.toLowerCase() <= "z" ? `Key${key.toUpperCase()}` : key === "\"" ? "Quote" : "",
        shiftKey: key === "G" || key === "\"",
        bubbles: true,
      }));
    }

    const sentKeys = (): string[] => socket?.sent.map((value) => JSON.parse(value).keys).filter(Boolean) ?? [];
    expect(sentKeys()).toEqual(keys.slice(0, 1));
    for (let index = 1; index < keys.length; index += 1) {
      socket?.onmessage?.(new MessageEvent("message", { data: JSON.stringify({ type: "input_ack" }) }));
      expect(sentKeys()).toEqual(keys.slice(0, index + 1));
    }

    view.destroy();
    parent.remove();
    binding.dispose();
  });
});
