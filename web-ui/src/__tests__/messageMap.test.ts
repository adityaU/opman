import { describe, expect, it } from "vitest";
import type { Message } from "../types";
import { mapToSortedArray, mergeMessage } from "../hooks/sse/messageMap";

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

  it("filters identical same-timestamp history records", () => {
    const first = {
      ...message("assistant-1", [{ id: "text-1", type: "text", text: "done" }]),
      info: { role: "assistant" as const, messageID: "assistant-1", time: { created: 42 } },
    };
    const duplicate = {
      ...first,
      info: { ...first.info, messageID: "assistant-duplicate" },
    };
    const map = new Map([
      [first.info.messageID!, first],
      [duplicate.info.messageID!, duplicate],
    ]);

    expect(mapToSortedArray(map)).toHaveLength(1);
  });
});
