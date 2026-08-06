/**
 * Session isolation: one transcript never shows another session's messages.
 *
 * `activeSessionRef` and `messageMapRef` are two halves of one fact — which
 * conversation is on screen. Every SSE handler decides *whether* to write by
 * reading the ref and decides *where* to write by reading the map, so any moment
 * where they disagree splices one session's turns into another's transcript.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

const transcripts: Record<string, any[]> = {};

const message = (sessionID: string, id: string, text: string, time: number) => ({
  info: { role: "assistant", id, messageID: id, sessionID, time: { created: time } },
  parts: [{ type: "text", id: `${id}:0`, messageID: id, sessionID, text }],
});

class FakeEventSource {
  static instances: FakeEventSource[] = [];
  readyState = 1;
  listeners = new Map<string, ((e: any) => void)[]>();
  constructor() {
    FakeEventSource.instances.push(this);
  }
  addEventListener(type: string, fn: (e: any) => void) {
    const list = this.listeners.get(type) ?? [];
    list.push(fn);
    this.listeners.set(type, list);
  }
  dispatch(type: string, data: string) {
    for (const fn of this.listeners.get(type) ?? []) fn({ data });
  }
  close() {
    this.readyState = 2;
  }
}

vi.mock("../api", () => ({
  createEventsSSE: () => new FakeEventSource(),
  createSessionEventsSSE: () => new FakeEventSource(),
  parseOpenCodeEvent: (data: string) => JSON.parse(data),
  fetchAppState: async () => ({
    projects: [
      { name: "p", path: "/p", index: 0, active_session: "ses_a", sessions: [], busy_sessions: [] },
    ],
    active_project: 0,
    startup_ready: true,
  }),
  fetchSessionMessages: async (sid: string) => ({
    messages: transcripts[sid] ?? [],
    has_more: false,
    total: (transcripts[sid] ?? []).length,
  }),
  fetchSessionStats: async () => null,
  fetchThemePair: async () => null,
  fetchPending: async () => ({ permissions: [], questions: [] }),
}));

import { useSSE } from "../hooks/sse/useSSE";

const textsOf = (messages: any[]) =>
  messages.flatMap((m) => m.parts.map((p: any) => p.text)).filter(Boolean);

describe("session isolation", () => {
  beforeEach(() => {
    FakeEventSource.instances = [];
    transcripts.ses_a = [message("ses_a", "msg_a", "alpha turn", 1000)];
    transcripts.ses_b = [message("ses_b", "msg_b", "beta turn", 2000)];
  });

  it("does not carry the previous session's transcript into a newly created one", async () => {
    const { result } = renderHook(() => useSSE());

    // Open session A and let its transcript load.
    act(() => result.current.beginSessionSwitch("ses_a"));
    await waitFor(() => expect(textsOf(result.current.messages)).toEqual(["alpha turn"]));

    // The send path creates a session and refreshes against it directly
    // (chatLayoutHandlers → refreshMessages(sid)), which is the only route that
    // moves the active session without going through a hydrate.
    await act(async () => {
      await result.current.refreshMessages("ses_b");
    });

    expect(textsOf(result.current.messages)).toEqual(["beta turn"]);
  });

  it("leaves the session it switched away from untouched in the cache", async () => {
    const { result } = renderHook(() => useSSE());
    act(() => result.current.beginSessionSwitch("ses_a"));
    await waitFor(() => expect(textsOf(result.current.messages)).toEqual(["alpha turn"]));

    await act(async () => {
      await result.current.refreshMessages("ses_b");
    });
    // Coming back reads the cached map — the contamination is permanent if the
    // new session's fetch was merged into it.
    act(() => result.current.beginSessionSwitch("ses_a"));
    await waitFor(() => expect(textsOf(result.current.messages)).toEqual(["alpha turn"]));
  });

  it("routes live events to the session that is actually on screen", async () => {
    const { result } = renderHook(() => useSSE());
    act(() => result.current.beginSessionSwitch("ses_a"));
    await waitFor(() => expect(textsOf(result.current.messages)).toEqual(["alpha turn"]));

    await act(async () => {
      await result.current.refreshMessages("ses_b");
    });

    // A live event for the session left behind must not reach the new transcript.
    const sse = FakeEventSource.instances.find((s) => s.listeners.has("opencode"))!;
    await act(async () => {
      sse.dispatch(
        "opencode",
        JSON.stringify({
          type: "message.part.updated",
          properties: {
            sessionID: "ses_a",
            part: { type: "text", id: "msg_a2:0", messageID: "msg_a2", sessionID: "ses_a", text: "alpha follow-up" },
          },
        }),
      );
      await new Promise((r) => setTimeout(r, 30));
    });

    expect(textsOf(result.current.messages)).toEqual(["beta turn"]);
  });
});
