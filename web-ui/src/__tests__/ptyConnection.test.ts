/**
 * The terminal's line to its PTY: one WebSocket for keys and output, the old stream +
 * ordered POSTs only when the socket cannot carry it.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const writeToPty = vi.fn();
const sse = { addEventListener: vi.fn(), close: vi.fn() };
const createPtySSE = vi.fn(() => sse);
vi.mock("../terminal-panel/writes", () => ({ writeToPty: (...args: unknown[]) => writeToPty(...args) }));
vi.mock("../api", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("../api");
  return { ...actual, createPtySSE: (...args: unknown[]) => createPtySSE(...(args as [])) };
});

import { connectPty } from "../terminal-panel/ptyConnection";

class FakeSocket {
  static last: FakeSocket | null = null;
  static OPEN = 1;
  readyState = 0;
  binaryType = "blob";
  sent: Uint8Array[] = [];
  private listeners = new Map<string, ((event: unknown) => void)[]>();
  constructor(readonly url: string) {
    FakeSocket.last = this;
  }
  addEventListener(type: string, listener: (event: unknown) => void) {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }
  send(data: Uint8Array) {
    this.sent.push(data);
  }
  close() {
    this.readyState = 3;
  }
  fire(type: string, event: unknown = {}) {
    if (type === "open") this.readyState = 1;
    for (const listener of this.listeners.get(type) ?? []) listener(event);
  }
}

const text = (bytes: Uint8Array) => new TextDecoder().decode(bytes);

beforeEach(() => {
  vi.stubGlobal("WebSocket", FakeSocket);
  writeToPty.mockReset();
  createPtySSE.mockClear();
});
afterEach(() => vi.unstubAllGlobals());

describe("connectPty", () => {
  it("asks for replay on the socket and sends keys typed before it opened, in order", () => {
    const connection = connectPty("t1", true, vi.fn());
    const socket = FakeSocket.last!;
    expect(socket.url).toContain("/pty/ws?id=t1&replay=1");
    connection.send("ls");
    connection.send(" -la\r");
    expect(socket.sent).toEqual([]);
    socket.fire("open");
    connection.send("x");
    expect(socket.sent.map(text)).toEqual(["ls -la\r", "x"]);
    expect(writeToPty).not.toHaveBeenCalled();
  });

  it("hands binary output straight to the terminal", () => {
    const onOutput = vi.fn();
    connectPty("t1", false, onOutput);
    const socket = FakeSocket.last!;
    socket.fire("open");
    socket.fire("message", { data: new TextEncoder().encode("hi\r\n").buffer });
    expect(text(onOutput.mock.calls[0][0])).toBe("hi\r\n");
  });

  it("falls back to the stream and POSTs when the socket never opens, keeping queued keys", () => {
    const connection = connectPty("t1", true, vi.fn());
    connection.send("pwd\r");
    FakeSocket.last!.fire("close");
    // Never opened, so the fallback may still replay.
    expect(createPtySSE).toHaveBeenCalledWith("t1", true);
    expect(writeToPty).toHaveBeenCalledWith("t1", "pwd\r");
    connection.send("y");
    expect(writeToPty).toHaveBeenLastCalledWith("t1", "y");
  });

  it("does not replay again after a socket that had opened drops", () => {
    connectPty("t1", true, vi.fn());
    const socket = FakeSocket.last!;
    socket.fire("open");
    socket.fire("close");
    expect(createPtySSE).toHaveBeenCalledWith("t1", false);
  });

  it("a deliberate close does not start the fallback", () => {
    const connection = connectPty("t1", false, vi.fn());
    connection.close();
    FakeSocket.last!.fire("close");
    expect(createPtySSE).not.toHaveBeenCalled();
  });
});
