import { describe, expect, it, vi } from "vitest";
import type { OpenCodeEvent } from "../types";
import { handleOpenCodeEvent, type EventHandlerContext } from "../hooks/sse/eventHandler";
import type { MessageMap } from "../hooks/sse/messageMap";

/** Minimal context — only the message-routing collaborators are exercised. */
function makeContext(activeSession: string | null, messageMap: MessageMap = new Map()) {
  const flushMessages = vi.fn();
  const flushSubagentMessages = vi.fn();
  const notifySession = vi.fn();
  const ctx = {
    activeSessionRef: { current: activeSession },
    messageMapRef: { current: messageMap },
    subagentMapsRef: { current: new Map<string, MessageMap>() },
    sessionCacheRef: { current: new Map() },
    flushMessages,
    flushSubagentMessages,
    notifySession,
    dropSession: vi.fn(),
    refreshState: vi.fn(),
    updateSessionMeta: vi.fn(),
    setStats: vi.fn(),
    setSessionStatus: vi.fn(),
    setBusySessions: vi.fn(),
    setSessionStatuses: vi.fn(),
    setPermissions: vi.fn(),
    setQuestions: vi.fn(),
    setCrossSessionPermissions: vi.fn(),
    setCrossSessionQuestions: vi.fn(),
    setFileEditCount: vi.fn(),
    resolvedQuestionIdsRef: { current: new Set<string>() },
  } as unknown as EventHandlerContext;
  return { ctx, flushMessages, flushSubagentMessages, notifySession };
}

const userMessageEvent = (sessionID: string, id: string, time: number): OpenCodeEvent =>
  ({
    type: "message.updated",
    properties: { sessionID, info: { id, role: "user", sessionID, time: { created: time } } },
  }) as unknown as OpenCodeEvent;

const userPartEvent = (sessionID: string, id: string, text: string): OpenCodeEvent =>
  ({
    type: "message.part.updated",
    properties: {
      sessionID,
      part: { type: "text", id: `${id}:0`, messageID: id, sessionID, text },
    },
  }) as unknown as OpenCodeEvent;

const placeholder = (sessionID: string, text: string, time: number) => ({
  info: { role: "user" as const, messageID: "__optimistic__1", id: "__optimistic__1", sessionID, time },
  parts: [{ type: "text", text }],
});

describe("handleOpenCodeEvent message routing", () => {
  it("ignores another session's user message while no session is active", () => {
    // The new-session screen: a prompt has been submitted and is showing as a
    // placeholder, but the server has not reported the new session active yet.
    // A background session's traffic must not disturb this transcript.
    const map: MessageMap = new Map([["__optimistic__1", placeholder("ses_mine", "hello", 1000)]]);
    const { ctx, flushMessages } = makeContext(null, map);

    handleOpenCodeEvent(ctx, userMessageEvent("ses_other", "msg_other", 2000));

    expect([...ctx.messageMapRef.current.keys()]).toEqual(["__optimistic__1"]);
    expect(flushMessages).not.toHaveBeenCalled();
  });

  it("ignores another session's user message while a session is active", () => {
    const map: MessageMap = new Map([["__optimistic__1", placeholder("ses_mine", "hello", 1000)]]);
    const { ctx } = makeContext("ses_mine", map);

    handleOpenCodeEvent(ctx, userMessageEvent("ses_other", "msg_other", 2000));

    expect([...ctx.messageMapRef.current.keys()]).toEqual(["__optimistic__1"]);
  });

  /**
   * Confirmation arrives in two frames: the envelope, then the text. Only the second one
   * proves which prompt was written, so the placeholder has to survive the first. Deciding
   * on the envelope alone meant falling back to comparing the browser's clock against the
   * server's, which kept the placeholder forever whenever the browser ran ahead.
   */
  it("keeps the placeholder until the confirmed message carries its text", () => {
    const map: MessageMap = new Map([["__optimistic__1", placeholder("ses_mine", "hello", 1000)]]);
    const { ctx } = makeContext("ses_mine", map);

    handleOpenCodeEvent(ctx, userMessageEvent("ses_mine", "msg_mine", 2000));

    expect([...ctx.messageMapRef.current.keys()]).toEqual(["__optimistic__1", "msg_mine"]);
  });

  it("retires the placeholder when this session's own message is confirmed", () => {
    const map: MessageMap = new Map([["__optimistic__1", placeholder("ses_mine", "hello", 1000)]]);
    const { ctx, flushMessages } = makeContext("ses_mine", map);

    handleOpenCodeEvent(ctx, userMessageEvent("ses_mine", "msg_mine", 2000));
    handleOpenCodeEvent(ctx, userPartEvent("ses_mine", "msg_mine", "hello"));

    expect([...ctx.messageMapRef.current.keys()]).toEqual(["msg_mine"]);
    expect(flushMessages).toHaveBeenCalled();
  });
});

/**
 * Panes read a session through the store, not through the hook's own
 * `messages`, so an event that mutates a session must wake that session's
 * watchers — including when the session is not the active one.
 */
describe("per-session notification", () => {
  it("notifies for the active session", () => {
    const { ctx, notifySession } = makeContext("s1");
    handleOpenCodeEvent(ctx, userMessageEvent("s1", "m1", 1));
    expect(notifySession).toHaveBeenCalledWith("s1");
  });

  it("notifies for a background session, which is the whole point", () => {
    const { ctx, notifySession } = makeContext("s1");
    handleOpenCodeEvent(ctx, userMessageEvent("s2", "m1", 1));
    expect(notifySession).toHaveBeenCalledWith("s2");
  });

  it("still notifies when a branch throws, so one bad event cannot freeze a pane", () => {
    const { ctx, notifySession } = makeContext("s1");
    ctx.messageMapRef = {
      get current(): never {
        throw new Error("boom");
      },
    } as unknown as typeof ctx.messageMapRef;

    expect(() => handleOpenCodeEvent(ctx, userMessageEvent("s1", "m1", 1))).toThrow("boom");
    expect(notifySession).toHaveBeenCalledWith("s1");
  });
});
