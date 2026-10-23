import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { BashOutput } from "../tool-call/BashOutput";
import { BashCard } from "../tool-call/readBashCards";
import { ToolOutput } from "../tool-call/components";
import { ansiChunks } from "../tool-call/ansiOutput";
import { bashOutputLanguage, stripTerminalControls } from "../tool-call/bashOutputFormat";

vi.mock("../hooks/useAutoOpen", () => ({
  useAutoOpen: () => ({ shouldAutoOpen: () => true }),
}));
afterEach(cleanup);

describe("Bash output", () => {
  it("highlights source printed by cat using the source language", () => {
    const { container } = render(<BashCard part={{ type: "tool", tool: "Bash", state: {
      status: "completed", input: { command: 'cat "src/main.rs"' }, output: "let count = 3;",
    } }} />);
    expect(container.querySelector(".bash-output .token.keyword")).toHaveTextContent("let");
    expect(container.querySelector(".bash-output .token.number")).toHaveTextContent("3");
  });
  it("highlights JSON output without changing whitespace", () => {
    const output = '{ "count": 3 }\n';
    const { container } = render(<BashOutput output={output} command="curl localhost" />);
    expect(container.querySelector(".token.number")).toHaveTextContent("3");
    expect(container.textContent).toBe(output);
  });
  it("uses themed ANSI styles, combines codes and resets them", () => {
    const chunks = ansiChunks("\x1b[1;31mfailed\x1b[22;39m normal");
    expect(chunks).toEqual([
      { text: "failed", style: { color: "var(--color-error)", fontWeight: 700 } },
      { text: " normal", style: {} },
    ]);
  });
  it("handles background, indexed and RGB colour sequences", () => {
    const chunks = ansiChunks("\x1b[48;5;2mgreen\x1b[49;38;2;30;40;50mRGB\x1b[0mend");
    expect(chunks[0].style.backgroundColor).toBe("var(--color-success)");
    expect(chunks[1].style).toEqual({ color: "rgb(30, 40, 50)" });
    expect(chunks[2].style).toEqual({});
  });
  it("strips OSC links, cursor controls and unfinished streaming escapes", () => {
    expect(stripTerminalControls("\x1b]8;;https://example.com\x07link\x1b]8;;\x07\x1b[2K\x1b[31")).toBe("link");
  });
  it("keeps coloured git diffs background-only", () => {
    const { container } = render(<BashOutput command="git diff"
      output={"diff --git a/a.ts b/a.ts\n\x1b[31m-const a = 1;\x1b[0m\n\x1b[32m+const a = 2;\x1b[0m\n"} />);
    expect(container.querySelector(".bash-output-diff-line.removed")).toHaveTextContent("-const a = 1;");
    expect(container.querySelector(".bash-output-diff-line.added")).toHaveTextContent("+const a = 2;");
    expect(container.querySelector("[style*='--color-error']")).toBeNull();
    expect(container.textContent).not.toContain("\x1b");
  });
  it("renders ANSI text safely without HTML injection", () => {
    const { container } = render(<BashOutput output={"\x1b[32m<script>alert(1)</script>\x1b[0m"} />);
    expect(container.querySelector("script")).toBeNull();
    expect(container.textContent).toBe("<script>alert(1)</script>");
    expect(container.querySelector("span")).toHaveStyle({ color: "var(--color-success)" });
  });
  it("uses the same renderer for background output", () => {
    const { container } = render(<ToolOutput toolName="bash" output="return 3"
      command="cat main.py" isLive />);
    expect(container.querySelector(".bash-output.tool-call-live-output .token.keyword")).toHaveTextContent("return");
  });
  it("keeps very large output as text", () => {
    const output = "long output ".repeat(20_000);
    const { container } = render(<BashOutput output={output} />);
    expect(container.textContent).toBe(output);
    expect(container.querySelector(".token")).toBeNull();
  });
  it("does not guess a source language through a pipeline", () => {
    expect(bashOutputLanguage("test", "cat a.py | head main.rs")).toBe("bash");
    expect(bashOutputLanguage("test", "cat a.py; cat main.rs")).toBe("bash");
  });
});
