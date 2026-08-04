import { describe, expect, it } from "vitest";
import type { Message } from "../types";
import { mergeMessage } from "../hooks/sse/messageMap";

const message = (id: string, parts: Message["parts"]): Message => ({
  info: { role: "assistant", messageID: id },
  parts,
});

describe("mergeMessage", () => {
  it("keeps cached parts and adds parts loaded from history", () => {
    const cached = message("assistant-1", [
      { id: "tool-1", type: "tool", tool: "edit" },
    ]);
    const history = message("assistant-1", [
      { id: "tool-1", type: "tool", tool: "edit", state: { status: "completed" } },
      { id: "tool-2", type: "tool", tool: "bash", state: { status: "completed" } },
    ]);

    const merged = mergeMessage(cached, history);

    expect(merged.parts).toHaveLength(2);
    expect(merged.parts.map((part) => part.id)).toEqual(["tool-1", "tool-2"]);
    expect(merged.parts[0]?.state?.status).toBe("completed");
  });
});
