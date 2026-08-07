import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  dropSession,
  getSessionView,
  isSessionPinned,
  pinnedSessions,
  publishSession,
  resetSessionStore,
  setSessionDemandHandler,
  subscribeSession,
} from "../hooks/sse/sessionStore";
import { SESSION_BUSY, SESSION_IDLE } from "../hooks/sse/types";
import type { Message } from "../types";

const message = (id: string): Message =>
  ({ info: { role: "user", messageID: id, sessionID: "s1" }, parts: [] }) as unknown as Message;

const view = (messages: Message[], status = SESSION_IDLE) => ({
  messages,
  stats: null,
  status,
  loading: false,
  hasOlder: false,
  total: messages.length,
});

beforeEach(resetSessionStore);

describe("subscription", () => {
  it("starts a cold session as loading with nothing in it", () => {
    const listener = vi.fn();
    subscribeSession("s1", listener);
    expect(getSessionView("s1")).toMatchObject({ messages: [], loading: true });
  });

  it("asks for hydration once, on the first subscriber only", () => {
    const demand = vi.fn();
    setSessionDemandHandler(demand);

    const first = subscribeSession("s1", vi.fn());
    subscribeSession("s1", vi.fn());

    expect(demand).toHaveBeenCalledTimes(1);
    expect(demand).toHaveBeenCalledWith("s1");
    first();
  });

  it("asks again after the last watcher leaves and a new one arrives", () => {
    const demand = vi.fn();
    setSessionDemandHandler(demand);
    subscribeSession("s1", vi.fn())();
    subscribeSession("s1", vi.fn());
    expect(demand).toHaveBeenCalledTimes(2);
  });

  it("wakes every subscriber of that session and nobody else", () => {
    const mine = vi.fn();
    const theirs = vi.fn();
    subscribeSession("s1", mine);
    subscribeSession("s2", theirs);

    publishSession("s1", view([message("m1")]));

    expect(mine).toHaveBeenCalledTimes(1);
    expect(theirs).not.toHaveBeenCalled();
  });

  it("stops waking a listener that unsubscribed", () => {
    const listener = vi.fn();
    const off = subscribeSession("s1", listener);
    off();
    publishSession("s1", view([message("m1")]));
    expect(listener).not.toHaveBeenCalled();
  });

  it("keeps one pane live when a second pane on the same session closes", () => {
    const stays = vi.fn();
    subscribeSession("s1", stays);
    const off = subscribeSession("s1", vi.fn());
    off();

    expect(isSessionPinned("s1")).toBe(true);
    publishSession("s1", view([message("m1")]));
    expect(stays).toHaveBeenCalledTimes(1);
  });
});

describe("publishing", () => {
  it("ignores a session nobody is watching, so events cost nothing", () => {
    publishSession("ghost", view([message("m1")]));
    expect(getSessionView("ghost").messages).toEqual([]);
  });

  /**
   * useSyncExternalStore compares by reference, so an equal-but-new snapshot
   * would re-render every pane on every frame of streamed output.
   */
  it("does not wake anyone for an identical snapshot", () => {
    const listener = vi.fn();
    subscribeSession("s1", listener);
    const next = view([message("m1")]);

    publishSession("s1", next);
    publishSession("s1", { ...next });

    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("wakes when only the status changed", () => {
    const listener = vi.fn();
    subscribeSession("s1", listener);
    const messages = [message("m1")];

    publishSession("s1", view(messages));
    publishSession("s1", view(messages, SESSION_BUSY));

    expect(listener).toHaveBeenCalledTimes(2);
    expect(getSessionView("s1").status).toBe(SESSION_BUSY);
  });
});

describe("pinning", () => {
  it("reports watched sessions so the LRU cache can skip them", () => {
    subscribeSession("s1", vi.fn());
    subscribeSession("s2", vi.fn());
    expect([...pinnedSessions()].sort()).toEqual(["s1", "s2"]);
  });

  it("unpins once the last watcher leaves", () => {
    const off = subscribeSession("s1", vi.fn());
    off();
    expect(isSessionPinned("s1")).toBe(false);
  });

  it("keeps the last snapshot for a reopened pane rather than flashing empty", () => {
    const off = subscribeSession("s1", vi.fn());
    publishSession("s1", view([message("m1")]));
    off();
    expect(getSessionView("s1").messages).toHaveLength(1);
  });
});

describe("deletion", () => {
  it("empties the view and tells watchers when a session is deleted upstream", () => {
    const listener = vi.fn();
    subscribeSession("s1", listener);
    publishSession("s1", view([message("m1")]));
    listener.mockClear();

    dropSession("s1");

    expect(listener).toHaveBeenCalledTimes(1);
    expect(getSessionView("s1")).toMatchObject({ messages: [] });
  });

  it("is a no-op for a session it never knew about", () => {
    expect(() => dropSession("ghost")).not.toThrow();
  });
});
