import React from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { CodeModeCard } from "../tool-call/CodeModeCard";
import { isCodeModeTool, parseCodeResult } from "../tool-call/codeModeParse";
import { CodeScript } from "../tool-call/CodeScript";
import { nestToolParts } from "../message-turn/nestToolParts";
import { toolDisplayName, toolType } from "../message-turn/toolLabels";
import type { MessagePart } from "../types";

vi.mock("../hooks/useAutoOpen", () => ({ useAutoOpen: () => ({ shouldAutoOpen: () => true }) }));

const capped = (text: string) => ({ text, bytes: text.length, truncated: false });
const record = (tool: string, output: string) => ({ tool, input: capped("{}"), output: capped(output),
  ok: true, finished: true, durationMs: 1 });
const part = (id: string, tool: string, parent?: string): MessagePart => ({
  type: "tool", callID: id, tool,
  state: { status: "completed", input: {}, output: "done", metadata: parent ? { parentCallID: parent } : {} },
});
const entries = (...parts: MessagePart[]) => parts.map((part, msgIdx) => ({ part, msgIdx }));

describe("Node REPL code mode", () => {
  it("recognizes MCP aliases and uses a plain group label", () => {
    for (const name of ["mcp__node_repl__js", "node_repl_js", "node_repl.js", "mcp__node-repl__js"]) {
      expect(isCodeModeTool(name)).toBe(true);
      expect(toolDisplayName(part("1", name), toolType(part("1", name)))).toBe("Code mode");
    }
    expect(isCodeModeTool("mcp__node_repl__reset")).toBe(false);
  });

  it("highlights freeform JS and the real Codex MCP argument wrapper", () => {
    const code = "const answer = await Promise.resolve(42);";
    const { container, rerender } = render(<CodeScript input={code} />);
    expect(container.querySelector(".token.keyword")).toHaveTextContent("const");
    rerender(<CodeScript input={{ server: "node_repl", tool: "js", arguments: { code } }} />);
    expect(container.querySelector(".token.keyword")).toHaveTextContent("const");
    expect(container).toHaveTextContent(code);
    expect(container).not.toHaveTextContent("arguments");
  });

  it("does not report zero internal calls when the upstream did not supply a log", () => {
    render(<CodeModeCard part={{ ...part("node", "mcp__node_repl__js"), state: {
      status: "completed", input: { code: "console.log('hello')" }, output: "hello",
    } }} />);
    expect(screen.getByText("JavaScript")).toBeTruthy();
    expect(screen.getByText("Code mode")).toBeTruthy();
    expect(screen.queryByText("0 tool calls")).toBeNull();
    expect(screen.queryByRole("region", { name: "Internal tool calls" })).toBeNull();
  });

  it("unwraps genuine result envelopes and renders recorded nested calls", async () => {
    const inner = { version: 1, calls: [record("opman_tools.grep", "found the row")] };
    const outer = { version: 1, calls: [record("code.execute", JSON.stringify({ result: {
      content: [{ type: "text", text: "inner result" }], structuredContent: inner,
    } }))] };
    expect(parseCodeResult(JSON.stringify({ result: { structuredContent: inner } })).calls).toEqual(inner.calls);
    render(<CodeModeCard part={{ ...part("node", "mcp__node_repl__js"), state: {
      status: "completed", metadata: { structuredContent: outer },
    } }} />);
    fireEvent.click(screen.getByText("code.execute"));
    expect(await screen.findByText("Grep")).toBeTruthy();
    fireEvent.click(screen.getByText("Grep"));
    expect(await screen.findByText("found the row")).toBeTruthy();
  });

  it("keeps every user-only call log and does not duplicate its structured copy", () => {
    const log = { version: 1, calls: [record("opman_tools.grep", "found")] };
    const marker = `<opman-code-calls>${JSON.stringify(log)}</opman-code-calls>`;
    expect(parseCodeResult(`first\n${marker}\nsecond\n${marker}`, log))
      .toEqual({ output: "first\n\nsecond", calls: [...log.calls, ...log.calls] });
  });

  it("places explicitly linked MCP children inside the code card", () => {
    const parent = part("p", "mcp__node_repl__js");
    const child = part("c", "mcp__opman-tools__grep", "p");
    child.state = { ...child.state, status: "completed", input: { pattern: "needle" }, output: "file.ts:2:needle" };
    const nested = nestToolParts(entries(parent, child));
    const { container } = render(<CodeModeCard part={nested[0].part} />);
    expect(screen.getByText("1 tool call")).toBeTruthy();
    expect(container.querySelector(".gmc-card-body .code-mode-children")).toHaveTextContent("Grep");
  });
});

describe("explicit internal tool links", () => {
  it("nests real children at multiple levels without changing stored parts", () => {
    const parent = part("p", "mcp__node_repl__js");
    const child = part("c", "code_execute", "p");
    const leaf = part("l", "mcp__opman-tools__read_file", "c");
    const nested = nestToolParts(entries(parent, child, leaf));
    expect(nested).toHaveLength(1);
    expect(nested[0].part.toolChildren?.[0].toolChildren?.[0]).toBe(leaf);
    expect(parent.toolChildren).toBeUndefined();
  });

  it("keeps siblings flat and leaves missing parents visible", () => {
    const raw = entries(part("p", "mcp__node_repl__js"), part("c", "read_file", "missing"));
    expect(nestToolParts(raw)).toHaveLength(2);
    expect(nestToolParts(raw).every(entry => !entry.part.toolChildren)).toBe(true);
  });

  it("handles child-first streams and cyclic metadata without losing calls", () => {
    const nested = nestToolParts(entries(part("c", "read_file", "p"), part("p", "mcp__node_repl__js")));
    expect(nested).toHaveLength(1);
    expect(nested[0].part.toolChildren?.[0].callID).toBe("c");
    const cycle = nestToolParts(entries(part("a", "code_execute", "b"), part("b", "read_file", "a")));
    const ids = cycle.flatMap(entry => [entry.part.callID, ...entry.part.toolChildren?.map(child => child.callID) ?? []]);
    expect(ids.sort()).toEqual(["a", "b"]);
  });
});
