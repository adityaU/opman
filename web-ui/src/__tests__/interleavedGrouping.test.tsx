/**
 * A run of same-tool calls must group on what the reader can see, not on what
 * the runner happened to put between them.
 *
 * The real transcripts that exposed this: two Bash calls in adjacent assistant
 * messages grouped, while the identical pair grouped *nothing* when the model
 * emitted an empty `thinking` block before the second one. Nothing about that
 * block is visible, so the grouping read as random.
 */
import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import { renderInterleavedContent } from "../message-turn/InterleavedContent";
import type { MessagePart } from "../message-turn/types";

vi.mock("../ToolCall", () => ({
  ToolCall: ({ part }: { part: MessagePart }) => (
    <div data-testid="tool-card">{(part as unknown as Record<string, unknown>).tool as string}</div>
  ),
}));

function tool(id: string, name: string): MessagePart {
  return {
    type: "tool", id, callID: id, tool: name,
    state: { status: "completed", input: {}, output: "ok", time: { start: 0, end: 10 } },
  } as unknown as MessagePart;
}

const soft = (type: string, text: string) => ({ type, text } as unknown as MessagePart);

function draw(parts: MessagePart[]) {
  const wrapped = parts.map((part, msgIdx) => ({ part, msgIdx }));
  return render(<div>{renderInterleavedContent(wrapped, [])}</div>);
}

describe("tool run grouping", () => {
  it("groups consecutive calls of the same tool", () => {
    const { container, getAllByTestId } = draw([tool("a", "Bash"), tool("b", "Bash")]);
    expect(container.querySelectorAll(".tool-run")).toHaveLength(1);
    expect(container.querySelector(".tool-run-count")?.textContent).toBe("2");
    // Collapsed: the calls themselves are not rendered until it is opened.
    expect(() => getAllByTestId("tool-card")).toThrow();
  });

  it("still groups them across an empty thinking block", () => {
    const { container } = draw([tool("a", "Bash"), soft("thinking", ""), tool("b", "Bash")]);
    expect(container.querySelectorAll(".tool-run")).toHaveLength(1);
    expect(container.querySelector(".tool-run-count")?.textContent).toBe("2");
  });

  it("still groups them across an empty text block", () => {
    const { container } = draw([tool("a", "Bash"), soft("text", "   "), tool("b", "Bash")]);
    expect(container.querySelectorAll(".tool-run")).toHaveLength(1);
  });

  it("still groups them across a tool result part", () => {
    const { container } = draw([
      tool("a", "Bash"),
      { type: "tool-result", toolCallId: "a", result: "ok" } as unknown as MessagePart,
      tool("b", "Bash"),
    ]);
    expect(container.querySelectorAll(".tool-run")).toHaveLength(1);
  });

  it("does not group across prose the reader can see", () => {
    const { container, getAllByTestId } = draw([
      tool("a", "Bash"),
      soft("text", "Now the second half:"),
      tool("b", "Bash"),
    ]);
    // Two standalone cards, no run summary: something real sits between them.
    expect(container.querySelectorAll(".tool-run")).toHaveLength(0);
    expect(getAllByTestId("tool-card")).toHaveLength(2);
  });

  it("does not group different tools", () => {
    const { container, getAllByTestId } = draw([tool("a", "Bash"), tool("b", "Read")]);
    expect(container.querySelectorAll(".tool-run")).toHaveLength(0);
    expect(getAllByTestId("tool-card")).toHaveLength(2);
  });

  it("never groups ui_render calls — the blocks are the output", () => {
    const { container, getAllByTestId } = draw([
      tool("a", "mcp__ui__ui_render"),
      tool("b", "mcp__ui__ui_render"),
    ]);
    expect(container.querySelectorAll(".tool-run")).toHaveLength(0);
    expect(getAllByTestId("tool-card")).toHaveLength(2);
  });

  it("renders no empty paragraph for a blank text block", () => {
    const { container } = draw([soft("text", "  \n "), tool("a", "Bash")]);
    expect(container.querySelectorAll(".message-body")).toHaveLength(0);
  });
});
