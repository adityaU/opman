import { describe, it, expect } from "vitest";
import { parseHandoffBlock, parseMemoryBlock } from "../message-turn/helpers";

const block = (transcript: string, userText: string, runner = "claude-code") =>
  [
    "[Handoff transcript]",
    `You are continuing a coding session handed over from the ${runner} runner. The turns below are the conversation so far — treat them as your own history, and do not repeat work already done.`,
    "",
    transcript,
    "[End handoff transcript]",
    "",
    userText,
  ].join("\n");

describe("parseHandoffBlock", () => {
  it("returns null for ordinary messages", () => {
    expect(parseHandoffBlock("just a message")).toBeNull();
  });

  it("returns null when the block is never closed", () => {
    expect(parseHandoffBlock("[Handoff transcript]\nno end")).toBeNull();
  });

  it("splits the transcript from the user's own text", () => {
    const parsed = parseHandoffBlock(
      block("--- user ---\nfix the build\n\n--- assistant ---\ndone", "now add tests"),
    );
    expect(parsed).not.toBeNull();
    expect(parsed!.fromRunner).toBe("claude-code");
    expect(parsed!.userText).toBe("now add tests");
    expect(parsed!.transcript).toBe(
      "--- user ---\nfix the build\n\n--- assistant ---\ndone",
    );
    // The model-facing lead-in is context for the runner, not for the reader.
    expect(parsed!.transcript).not.toContain("You are continuing");
  });

  it("leaves a session-instructions block for the memory parser", () => {
    const inner = "[Session instructions]\n- Tone: terse\n\n[User request]\nnow add tests";
    const parsed = parseHandoffBlock(block("--- user ---\nhi", inner));
    expect(parsed!.userText).toBe(inner);
    const memory = parseMemoryBlock(parsed!.userText);
    expect(memory!.userText).toBe("now add tests");
    expect(memory!.items).toEqual([{ label: "Tone", content: "terse" }]);
  });

  it("tolerates a missing runner name", () => {
    const raw = "[Handoff transcript]\nsomething odd\n[End handoff transcript]\n\nhello";
    const parsed = parseHandoffBlock(raw);
    expect(parsed!.fromRunner).toBe("");
    expect(parsed!.userText).toBe("hello");
  });
});
