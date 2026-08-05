import { describe, expect, it } from "vitest";
import type { Message } from "../types";
import type { MessageMap } from "../hooks/sse/messageMap";
import {
  createOptimisticId,
  isOptimisticId,
  purgeOptimistic,
  reconcileOptimistic,
  retainOptimistic,
} from "../hooks/sse/optimistic";

const userMessage = (id: string, sessionID: string, text: string, time: number): Message => ({
  info: { role: "user", messageID: id, id, sessionID, time },
  parts: [{ type: "text", text }],
});

const mapOf = (...messages: Message[]): MessageMap =>
  new Map(messages.map((msg) => [msg.info.messageID!, msg]));

describe("createOptimisticId", () => {
  it("produces ids recognised as optimistic", () => {
    expect(isOptimisticId(createOptimisticId())).toBe(true);
    expect(isOptimisticId("msg_real")).toBe(false);
  });
});

describe("retainOptimistic", () => {
  it("keeps only this session's placeholders when switching in", () => {
    const map = mapOf(
      userMessage("__optimistic__1", "ses_new", "hello", 10),
      userMessage("__optimistic__2", "ses_other", "elsewhere", 10),
      userMessage("msg_real", "ses_new", "older turn", 5),
    );

    const kept = retainOptimistic(map, "ses_new");

    expect([...kept.keys()]).toEqual(["__optimistic__1"]);
  });

  it("keeps nothing when there is no target session", () => {
    const map = mapOf(userMessage("__optimistic__1", "ses_new", "hello", 10));
    expect(retainOptimistic(map, null).size).toBe(0);
  });
});

describe("reconcileOptimistic", () => {
  it("retires a placeholder once the transcript carries the same text", () => {
    const map = mapOf(
      userMessage("__optimistic__100", "ses_a", "run the tests", 100),
      userMessage("msg_real", "ses_a", "run the tests", 101),
    );

    expect(reconcileOptimistic(map)).toBe(true);
    expect([...map.keys()]).toEqual(["msg_real"]);
  });

  it("retires a placeholder a newer real message supersedes, even if reworded", () => {
    const map = mapOf(
      userMessage("__optimistic__100", "ses_a", "run the tests", 100),
      userMessage("msg_real", "ses_a", "<prompt>run the tests</prompt>", 101),
    );

    expect(reconcileOptimistic(map)).toBe(true);
    expect([...map.keys()]).toEqual(["msg_real"]);
  });

  it("keeps a placeholder the transcript has not caught up to yet", () => {
    // The claude --bg case: session exists, nothing written to the transcript.
    const map = mapOf(userMessage("__optimistic__100", "ses_a", "run the tests", 100));

    expect(reconcileOptimistic(map)).toBe(false);
    expect([...map.keys()]).toEqual(["__optimistic__100"]);
  });

  it("keeps a queued follow-up while retiring the confirmed message", () => {
    const map = mapOf(
      userMessage("__optimistic__100", "ses_a", "first", 100),
      userMessage("msg_real", "ses_a", "first", 101),
      userMessage("__optimistic__200", "ses_a", "queued follow-up", 200),
    );

    expect(reconcileOptimistic(map)).toBe(true);
    expect([...map.keys()]).toEqual(["msg_real", "__optimistic__200"]);
  });
});

describe("purgeOptimistic", () => {
  it("drops every placeholder and reports whether it changed anything", () => {
    const map = mapOf(
      userMessage("__optimistic__1", "ses_a", "a", 1),
      userMessage("msg_real", "ses_a", "b", 2),
    );

    expect(purgeOptimistic(map)).toBe(true);
    expect([...map.keys()]).toEqual(["msg_real"]);
    expect(purgeOptimistic(map)).toBe(false);
  });
});
