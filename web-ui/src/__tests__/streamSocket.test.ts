/**
 * Every event stream of a frontend instance rides one WebSocket: framing, dispatch to
 * EventSource-style listeners, reconnect + resubscribe, and cleanup.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MuxEventSource, openEventStream, resetStreamHubs } from "../api/streamSocket";
import { IDLE_GRACE_MS, RETRY_MS, SILENCE_LIMIT_MS } from "../api/streamHub";

class FakeWebSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  static instances: FakeWebSocket[] = [];
  readyState = FakeWebSocket.CONNECTING;
  sent: any[] = [];
  onopen: ((e: Event) => void) | null = null;
  onmessage: ((e: MessageEvent) => void) | null = null;
  onclose: ((e: Event) => void) | null = null;
  onerror: ((e: Event) => void) | null = null;
  constructor(readonly url: string) {
    FakeWebSocket.instances.push(this);
  }
  send(text: string) {
    this.sent.push(JSON.parse(text));
  }
  close() {
    this.readyState = FakeWebSocket.CLOSED;
  }
  // ── test side ──
  accept() {
    this.readyState = FakeWebSocket.OPEN;
    this.onopen?.(new Event("open"));
  }
  frame(value: unknown) {
    this.onmessage?.(new MessageEvent("message", { data: JSON.stringify(value) }));
  }
  drop() {
    this.readyState = FakeWebSocket.CLOSED;
    this.onclose?.(new Event("close"));
  }
  subs() {
    return this.sent.filter((f) => f.op === "sub");
  }
}

const last = () => FakeWebSocket.instances[FakeWebSocket.instances.length - 1];

beforeEach(() => {
  vi.useFakeTimers();
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
  resetStreamHubs();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("MuxEventSource", () => {
  it("shares one socket and frames sub/unsub", () => {
    const a = openEventStream("/api/events");
    const b = openEventStream("/api/pty/stream?id=x&replay=1");
    expect(a).toBeInstanceOf(MuxEventSource);
    expect(FakeWebSocket.instances).toHaveLength(1);
    const ws = last();
    expect(ws.url).toBe(`ws://${location.host}/api/stream/ws`);
    expect(ws.sent).toEqual([]);
    ws.accept();
    expect(ws.subs()).toEqual([
      { op: "sub", id: 1, path: "/api/events" },
      { op: "sub", id: 2, path: "/api/pty/stream?id=x&replay=1" },
    ]);
    const c = openEventStream("/api/system/stats/stream");
    expect(ws.sent.at(-1)).toEqual({ op: "sub", id: 3, path: "/api/system/stats/stream" });
    b.close();
    expect(ws.sent.at(-1)).toEqual({ op: "unsub", id: 2 });
    expect(b.readyState).toBe(2);
    a.close();
    c.close();
  });

  it("routes a remote instance's streams to that server's socket", () => {
    openEventStream("/remote/dev/api/session/events");
    expect(last().url).toBe(`ws://${location.host}/remote/dev/api/stream/ws`);
    last().accept();
    expect(last().subs()).toEqual([{ op: "sub", id: 1, path: "/api/session/events" }]);
  });

  it("dispatches open, named events and messages like EventSource", () => {
    const source = openEventStream("/api/events");
    const ws = last();
    ws.accept();
    const seen: string[] = [];
    source.onopen = () => seen.push("onopen");
    source.addEventListener("open", () => seen.push("open"));
    source.addEventListener("session_busy", (e) => seen.push(`busy:${(e as MessageEvent).data}`));
    source.onmessage = (e) => seen.push(`onmessage:${e.data}:${e.lastEventId}`);
    const removed = () => seen.push("removed");
    source.addEventListener("session_busy", removed);
    source.removeEventListener("session_busy", removed);

    ws.frame({ id: 1, op: "open" });
    expect(source.readyState).toBe(1);
    ws.frame({ id: 1, event: "session_busy", data: "ses_1" });
    ws.frame({ id: 1, event: "message", data: "a\nb", lastId: "7" });
    ws.frame({ id: 99, event: "message", data: "someone else's" });
    ws.frame({ op: "ping" });
    expect(seen).toEqual(["onopen", "open", "busy:ses_1", "onmessage:a\nb:7"]);
  });

  it("a dropped socket errors every source, then reconnects and resubscribes", () => {
    const a = openEventStream("/api/events");
    const b = openEventStream("/api/session/events");
    const first = last();
    first.accept();
    first.frame({ id: 1, op: "open" });
    first.frame({ id: 2, op: "open" });
    const errors: string[] = [];
    a.addEventListener("error", () => errors.push("a"));
    b.onerror = () => errors.push("b");

    first.drop();
    expect(errors).toEqual(["a", "b"]);
    expect(a.readyState).toBe(0);
    expect(FakeWebSocket.instances).toHaveLength(1);

    vi.advanceTimersByTime(1000);
    const second = last();
    expect(second).not.toBe(first);
    second.accept();
    expect(second.subs().map((f) => f.path)).toEqual(["/api/events", "/api/session/events"]);
    let reopened = 0;
    a.addEventListener("open", () => reopened++);
    second.frame({ id: 1, op: "open" });
    expect(reopened).toBe(1);
    expect(a.readyState).toBe(1);
  });

  it("a silent socket is treated as dead", () => {
    openEventStream("/api/events");
    const first = last();
    first.accept();
    vi.advanceTimersByTime(SILENCE_LIMIT_MS + SILENCE_LIMIT_MS / 4 + 1);
    expect(first.readyState).toBe(FakeWebSocket.CLOSED);
    vi.advanceTimersByTime(1000);
    expect(FakeWebSocket.instances).toHaveLength(2);
  });

  it("a refused stream closes; a finished one resubscribes after the retry delay", () => {
    const refused = openEventStream("/api/fake/json");
    const finished = openEventStream("/api/events");
    const ws = last();
    ws.accept();
    let errors = 0;
    refused.onerror = () => errors++;
    ws.frame({ id: 1, op: "end", status: 415 });
    expect(refused.readyState).toBe(2);
    expect(errors).toBe(1);

    ws.frame({ id: 2, op: "open" });
    ws.frame({ id: 2, op: "end" });
    expect(finished.readyState).toBe(0);
    vi.advanceTimersByTime(RETRY_MS);
    expect(ws.sent.at(-1)).toEqual({ op: "sub", id: 2, path: "/api/events" });
    ws.frame({ id: 2, op: "open" });
    expect(finished.readyState).toBe(1);
  });

  it("closes the socket after the last source closes, and reopens on demand", () => {
    const a = openEventStream("/api/events");
    const ws = last();
    ws.accept();
    a.close();
    vi.advanceTimersByTime(IDLE_GRACE_MS - 1);
    expect(ws.readyState).toBe(FakeWebSocket.OPEN);
    const b = openEventStream("/api/events");
    vi.advanceTimersByTime(IDLE_GRACE_MS);
    expect(ws.readyState).toBe(FakeWebSocket.OPEN);
    b.close();
    vi.advanceTimersByTime(IDLE_GRACE_MS);
    expect(ws.readyState).toBe(FakeWebSocket.CLOSED);
    openEventStream("/api/events");
    expect(FakeWebSocket.instances).toHaveLength(2);
  });

  it("falls back to a native EventSource without WebSocket", () => {
    const Native = vi.fn();
    vi.stubGlobal("WebSocket", undefined);
    vi.stubGlobal("EventSource", Native);
    openEventStream("/api/events");
    expect(Native).toHaveBeenCalledWith("/api/events");
  });
});
