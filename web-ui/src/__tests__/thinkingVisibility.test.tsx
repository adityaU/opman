import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { ThinkingVisibilitySetting } from "../settings-page/ThinkingVisibilitySetting";
import { SessionLine } from "../prompt-input/SessionLine";
import { renderInterleavedContent } from "../message-turn/InterleavedContent";
import type { MessagePart } from "../message-turn/types";
import { saveThinkingVisibility } from "../utils/thinkingVisibility";

function ThinkingFixture({ id }: { id: number }) {
  const parts = [
    { type: "text", text: "Assistant final response" },
    // OpenCode emits reasoning directly; ACP and Claude CLI normalize their
    // thought blocks to the same `reasoning` part shape before the UI sees it.
    { type: "reasoning", text: `**Reasoning heading ${id}**\n\nReasoning body ${id}` },
    { type: "text", text: "Assistant final response continues" },
    // Older stored payloads use the source-specific field names.
    { type: "thinking", thinking: `**Thinking heading ${id}**\n\nThinking body ${id}` },
    { type: "text", text: "Assistant final response ends" },
    { type: "analysis", analysis: `**Analysis heading ${id}**\n\nAnalysis body ${id}` },
  ] as unknown as MessagePart[];

  return <div>{renderInterleavedContent(parts.map((part, msgIdx) => ({ part, msgIdx })), [])}</div>;
}

function Harness({ id }: { id: number }) {
  return (
    <>
      <ThinkingVisibilitySetting />
      <SessionLine title="Build" sessionId="s1" busy progressText={`Progress heading ${id}`} />
      <ThinkingFixture id={id} />
    </>
  );
}

describe("thinking visibility preference", () => {
  beforeEach(() => localStorage.removeItem("opman-show-thinking"));

  it("hides live and archived thinking by default, then reveals and persists it on opt-in", () => {
    const { rerender } = render(<Harness id={1} />);
    const toggle = screen.getByRole("checkbox", { name: "Show thinking" }) as HTMLInputElement;

    expect(toggle.checked).toBe(false);
    expect(screen.getByText("Assistant final response")).toBeTruthy();
    expect(screen.getByText("Assistant final response continues")).toBeTruthy();
    expect(screen.getByText("Assistant final response ends")).toBeTruthy();
    expect(screen.getByText("Progress heading 1")).toBeTruthy();
    expect(screen.queryByText("Reasoning heading 1")).toBeNull();
    expect(screen.queryByText("Reasoning body 1")).toBeNull();
    expect(screen.queryByText("Thinking heading 1")).toBeNull();
    expect(screen.queryByText("Thinking body 1")).toBeNull();
    expect(screen.queryByText("Analysis heading 1")).toBeNull();
    expect(screen.queryByText("Analysis body 1")).toBeNull();

    fireEvent.click(toggle);
    expect(screen.getByText("Progress heading 1")).toBeTruthy();
    expect(screen.getByText("Reasoning heading 1")).toBeTruthy();
    expect(screen.getByText("Reasoning body 1")).toBeTruthy();
    expect(screen.getByText("Thinking heading 1")).toBeTruthy();
    expect(screen.getByText("Thinking body 1")).toBeTruthy();
    expect(screen.getByText("Analysis heading 1")).toBeTruthy();
    expect(screen.getByText("Analysis body 1")).toBeTruthy();

    fireEvent.click(toggle);
    expect(localStorage.getItem("opman-show-thinking")).toBe("false");
    expect(screen.getByText("Progress heading 1")).toBeTruthy();
    expect(screen.queryByText("Reasoning heading 1")).toBeNull();
    expect(screen.queryByText("Reasoning body 1")).toBeNull();
    expect(screen.queryByText("Thinking heading 1")).toBeNull();
    expect(screen.queryByText("Thinking body 1")).toBeNull();
    expect(screen.queryByText("Analysis heading 1")).toBeNull();
    expect(screen.queryByText("Analysis body 1")).toBeNull();

    rerender(<Harness id={2} />);
    expect((screen.getByRole("checkbox", { name: "Show thinking" }) as HTMLInputElement).checked).toBe(false);
    expect(screen.getByText("Progress heading 2")).toBeTruthy();
    expect(screen.queryByText("Reasoning heading 2")).toBeNull();
    expect(screen.queryByText("Thinking heading 2")).toBeNull();
    expect(screen.queryByText("Thinking body 2")).toBeNull();
    expect(screen.queryByText("Analysis heading 2")).toBeNull();
    expect(screen.queryByText("Analysis body 2")).toBeNull();
    expect(screen.getByText("Assistant final response")).toBeTruthy();
  });

  it("loads an explicitly saved opt-in", () => {
    localStorage.setItem("opman-show-thinking", "true");
    render(<Harness id={3} />);

    expect((screen.getByRole("checkbox", { name: "Show thinking" }) as HTMLInputElement).checked).toBe(true);
    expect(screen.getByText("Reasoning heading 3")).toBeTruthy();
    expect(screen.getByText("Reasoning body 3")).toBeTruthy();
    expect(screen.getByText("Assistant final response")).toBeTruthy();
    expect(screen.getByText("Assistant final response continues")).toBeTruthy();
    expect(screen.getByText("Assistant final response ends")).toBeTruthy();
  });

  it("applies the switch for this session when browser storage is unavailable", () => {
    render(<Harness id={4} />);
    const write = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("Storage unavailable");
    });
    try {
      fireEvent.click(screen.getByRole("checkbox", { name: "Show thinking" }));
      expect(screen.getByText("Reasoning heading 4")).toBeTruthy();
      expect(screen.getByText("Reasoning body 4")).toBeTruthy();
    } finally {
      write.mockRestore();
      saveThinkingVisibility(false);
    }
  });
});
