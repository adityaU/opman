import { describe, expect, it, vi } from "vitest";
import { sameStatus, withBusy, withStatus } from "../hooks/sse/statusWrites";
import { setupAppSSEListeners, type AppSSEContext } from "../hooks/sse/eventHandler";
import { SESSION_BUSY, SESSION_IDLE, type SessionStatus } from "../hooks/sse/types";

describe("sameStatus", () => {
  it("compares by type", () => {
    expect(sameStatus(SESSION_BUSY, { type: "busy" })).toBe(true);
    expect(sameStatus(SESSION_BUSY, SESSION_IDLE)).toBe(false);
  });

  it("treats a retry with a new attempt or clock as a different status", () => {
    const first: SessionStatus = { type: "retry", attempt: 1, message: "…", next: 10 };
    expect(sameStatus(first, { ...first })).toBe(true);
    expect(sameStatus(first, { ...first, attempt: 2 })).toBe(false);
    expect(sameStatus(first, { ...first, next: 20 })).toBe(false);
  });
});

describe("withStatus", () => {
  it("stores idle as absence", () => {
    const busy = withStatus({}, "s1", SESSION_BUSY);
    expect(busy.s1).toEqual(SESSION_BUSY);
    expect(withStatus(busy, "s1", SESSION_IDLE)).toEqual({});
  });

  it("returns the same map when nothing moved", () => {
    const busy = withStatus({}, "s1", SESSION_BUSY);
    expect(withStatus(busy, "s1", SESSION_BUSY)).toBe(busy);
    expect(withStatus({}, "s1", SESSION_IDLE)).toEqual({});
  });
});

describe("withBusy", () => {
  it("adds, removes, and keeps identity when unchanged", () => {
    const empty = new Set<string>();
    expect(withBusy(empty, "s1", false)).toBe(empty);
    const one = withBusy(empty, "s1", true);
    expect([...one]).toEqual(["s1"]);
    expect(withBusy(one, "s1", true)).toBe(one);
    expect([...withBusy(one, "s1", false)]).toEqual([]);
  });
});

/** An EventSource stand-in that just remembers its listeners. */
function fakeStream() {
  const listeners = new Map<string, (e: MessageEvent) => void>();
  const stream = {
    addEventListener: (name: string, fn: (e: MessageEvent) => void) => { listeners.set(name, fn); },
  } as unknown as EventSource;
  const emit = (name: string, data: string) => {
    listeners.get(name)?.({ data } as MessageEvent);
  };
  return { stream, emit };
}

describe("app stream busy/idle events", () => {
  it("routes both through the single status writer", () => {
    const applySessionStatus = vi.fn();
    const ctx = {
      activeSessionRef: { current: null },
      sessionCacheRef: { current: new Map() },
      refreshState: vi.fn(),
      touchEvent: vi.fn(),
      recoverAfterReconnect: vi.fn(),
      applySessionStatus,
      setStats: vi.fn(),
      setWatcherStatus: vi.fn(),
      setMcpEditorOpenPath: vi.fn(),
      setMcpEditorOpenLine: vi.fn(),
      setMcpTerminalFocusId: vi.fn(),
      setMcpBrowserOpen: vi.fn(),
      setMcpAgentActivity: vi.fn(),
      setPresenceClients: vi.fn(),
    } as unknown as AppSSEContext;

    const { stream, emit } = fakeStream();
    setupAppSSEListeners(stream, ctx);

    emit("session_busy", "s1");
    emit("session_idle", "s1");

    expect(applySessionStatus.mock.calls).toEqual([
      ["s1", SESSION_BUSY],
      ["s1", SESSION_IDLE],
    ]);
  });
});
