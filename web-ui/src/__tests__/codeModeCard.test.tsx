import React from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

  it("shows input, error, duration and truncation inside each inner call", async () => {
    render(<CodeModeCard part={{ type: "tool", tool: "code_execute", state: {
      status: "completed", input: { code: "return await tools.opman_tools.read_file({path:'a.rs'})" },
      output: marked,
    } }} />);
    expect(screen.getByText("1 tool call")).toBeTruthy();
    expect(screen.getByText("Error · 42ms")).toBeTruthy();
    fireEvent.click(screen.getByText("Read"));
    expect(await screen.findByText(input.text)).toBeTruthy();
    expect(screen.getByText(output.text)).toBeTruthy();
    expect(screen.getByText(/9999 bytes total/)).toBeTruthy();
  });

  it("highlights Bash stdout inside execute without showing its JSON envelope", async () => {
    const calls = [{ tool: "opman_tools.bash",
      input: { text: '{"command":"cat main.py"}', truncated: false, bytes: 25 },
      output: { text: '{"stdout":"return 3","stderr":"","exit_code":0}', truncated: false, bytes: 46 },
      ok: true, finished: true, durationMs: 3 }];
    const { container } = render(<CodeModeCard part={{ type: "tool", tool: "code_execute",
      state: { status: "completed", output: "", metadata: { structuredContent: { version: 1, calls } } },
    }} />);
    fireEvent.click(screen.getByText("Bash"));
    await waitFor(() => expect(container.querySelector(".bash-output")).toBeTruthy());
    expect(container.querySelector(".bash-output .token.keyword")).toHaveTextContent("return");
    expect(container.querySelector(".bash-output")).toHaveTextContent("return 3");
    expect(container.querySelector(".bash-output")).not.toHaveTextContent("stdout");
  });

  it("formats Grep and Edit inner tool calls without raw JSON envelopes", async () => {
    const calls = [
      {
        tool: "opman_tools.grep",
        input: { text: '{"pattern":"fn main","path":"src"}', truncated: false, bytes: 35 },
        output: {
          text: '{"mode":"content","results":[{"path":"src/main.rs","line":1,"text":"fn main() {}"}],"truncated":false}',
          truncated: false,
          bytes: 98,
        },
        ok: true,
        finished: true,
        durationMs: 4,
      },
    ];
    render(
      <CodeModeCard
        part={{
          type: "tool",
          tool: "code_execute",
          state: { status: "completed", output: "", metadata: { structuredContent: { version: 1, calls } } },
        }}
      />
    );
    expect(screen.getByText("fn main")).toBeTruthy();
    fireEvent.click(screen.getByText("Grep"));
    expect(await screen.findByText("src/main.rs:1:fn main() {}")).toBeTruthy();
  });
});
