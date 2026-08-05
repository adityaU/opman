import { describe, expect, it } from "vitest";
import type { Message, MessagePart } from "../types";
import { groupMessages } from "../message-timeline/types";

function message(
  id: string,
  role: Message["info"]["role"],
  parts: MessagePart[],
  agent?: string,
): Message {
  return { info: { id, role, agent }, parts };
}

describe("groupMessages", () => {
  it("keeps interleaved assistant text and tools in one agent turn", () => {
    const groups = groupMessages([
      message("text-1", "assistant", [{ type: "text", text: "Starting." }], "codex"),
      message("tools", "assistant", [{ type: "tool", tool: "bash", callID: "call-1" }], "codex"),
      message("text-2", "assistant", [{ type: "text", text: "Finished." }], "codex"),
    ]);

    expect(groups).toHaveLength(1);
    expect(groups[0].messages.map((item) => item.info.id)).toEqual(["text-1", "tools", "text-2"]);
  });

  it("starts a new agent turn after a user message", () => {
    const groups = groupMessages([
      message("assistant-1", "assistant", [{ type: "text", text: "First." }], "codex"),
      message("user", "user", [{ type: "text", text: "Continue." }]),
      message("assistant-2", "assistant", [{ type: "text", text: "Second." }], "codex"),
    ]);

    expect(groups.map((group) => group.role)).toEqual(["assistant", "user", "assistant"]);
  });

  it("does not merge an explicit agent handoff", () => {
    const groups = groupMessages([
      message("codex", "assistant", [{ type: "text", text: "I will hand this off." }], "codex"),
      message("claude", "assistant", [{ type: "text", text: "Taking over." }], "claude"),
    ]);

    expect(groups).toHaveLength(2);
  });
});
