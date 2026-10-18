import React from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { CodeModeCard } from "../tool-call/CodeModeCard";
import { isCodeModeTool, parseCodeResult } from "../tool-call/codeModeParse";

vi.mock("../hooks/useAutoOpen", () => ({ useAutoOpen: () => ({ shouldAutoOpen: () => true }) }));
vi.mock("../tool-call/GenericToolViews", () => ({
  InputView: ({ data }: { data: unknown }) => <pre>{JSON.stringify(data)}</pre>,
  OutputView: ({ output }: { output: string }) => <pre>{output}</pre>,
}));

const input = { text: '{"path":"a.rs"}', truncated: false, bytes: 15 };
const output = { text: "read failed: missing file", truncated: true, bytes: 9999 };
const log = { version: 1, calls: [{ tool: "opman_tools.read_file", input, output,
  ok: false, finished: true, durationMs: 42 }] };
const marked = `Result\n<opman-code-calls>${JSON.stringify(log)}</opman-code-calls>`;

describe("code mode output", () => {
  it("recognizes names from OpenCode, Claude and ACP", () => {
    for (const name of ["code_execute", "code__execute", "mcp__code__execute"]) {
      expect(isCodeModeTool(name)).toBe(true);
    }
    expect(isCodeModeTool("execute")).toBe(false);
    expect(isCodeModeTool("bash_execute")).toBe(false);
  });

  it("extracts user call records and keeps the script result", () => {
    expect(parseCodeResult(marked)).toEqual({ output: "Result", calls: log.calls });
  });

  it("supports MCP result envelopes and content arrays", () => {
    const content = [{ type: "text", text: marked }];
    expect(parseCodeResult(JSON.stringify(content)).calls).toEqual(log.calls);
    expect(parseCodeResult(JSON.stringify({ content, structuredContent: log })).output).toBe("Result");
    expect(parseCodeResult("Result", log).calls).toEqual(log.calls);
  });

  it("keeps invalid records visible without crashing", () => {
    const broken = "Result<opman-code-calls>{oops}</opman-code-calls>";
    expect(parseCodeResult(broken)).toEqual({ output: broken, calls: [] });
    expect(parseCodeResult("<opman-code-calls>{\"version\":1,\"calls\":[null]}</opman-code-calls>").calls).toEqual([]);
  });

  it("shows input, error, duration and truncation inside each inner call", () => {
    render(<CodeModeCard part={{ type: "tool", tool: "code_execute", state: {
      status: "completed", input: { code: "return await tools.opman_tools.read_file({path:'a.rs'})" },
      output: marked,
    } }} />);
    expect(screen.getByText("1 tool call")).toBeTruthy();
    expect(screen.getByText("Error · 42ms")).toBeTruthy();
    fireEvent.click(screen.getByText("opman_tools.read_file"));
    expect(screen.getByText(input.text)).toBeTruthy();
    expect(screen.getByText(output.text)).toBeTruthy();
    expect(screen.getByText(/9999 bytes total/)).toBeTruthy();
  });
});
