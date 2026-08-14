import { describe, expect, it } from "vitest";
import type { Message } from "../types";
import { activeProgressText } from "../prompt-input/progress";

const assistant = (parts: Message["parts"]): Message => ({
  info: { role: "assistant" },
  parts,
});

describe("activeProgressText", () => {
  it("returns the newest active tool title", () => {
    const messages = [assistant([
      { type: "tool", state: { status: "completed", title: "Old work" } },
      { type: "tool", state: { status: "running", title: "Planning memory update with apply_patch" } },
    ])];

    expect(activeProgressText(messages, true)).toBe("Planning memory update with apply_patch");
  });

  it("replaces an older title as the latest tool becomes active", () => {
    const messages = [assistant([
      { type: "tool", state: { status: "completed", title: "Old work" } },
      { type: "tool", state: { status: "pending", title: "Running tests" } },
    ])];

    expect(activeProgressText(messages, true)).toBe("Running tests");
  });

  it("does not keep completed progress in the composer", () => {
    expect(activeProgressText([
      assistant([{ type: "tool", state: { status: "completed", title: "Finished" } }]),
    ], true)).toBeNull();
  });

  it("shows nothing once the session is idle", () => {
    expect(activeProgressText([
      assistant([{ type: "reasoning", text: "**Exploring the repo**\n\nLooking around." }]),
    ], false)).toBeNull();
  });

  it("prefers the bold header a narrating runner emits", () => {
    expect(activeProgressText([
      assistant([{ type: "reasoning", text: "**Exploring the repo**\n\nLooking around." }]),
    ], true)).toBe("Exploring the repo");
  });

  it("tracks the latest header inside a streaming reasoning part", () => {
    expect(activeProgressText([
      assistant([{
        type: "reasoning",
        text: "**Exploring the repo**\n\nDone.\n\n**Wiring the composer**\n\nEditing.",
      }]),
    ], true)).toBe("Wiring the composer");
  });

  it("uses the newest part when a tool call follows the narration", () => {
    expect(activeProgressText([
      assistant([
        { type: "reasoning", text: "**Exploring the repo**" },
        { type: "tool", state: { status: "running", title: "Reading progress.ts" } },
      ]),
    ], true)).toBe("Reading progress.ts");
  });

  it("skips reasoning that carries no header", () => {
    expect(activeProgressText([
      assistant([
        { type: "tool", state: { status: "running", title: "Reading progress.ts" } },
        { type: "reasoning", text: "Still thinking about it." },
      ]),
    ], true)).toBe("Reading progress.ts");
  });
});
