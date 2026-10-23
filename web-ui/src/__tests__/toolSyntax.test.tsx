import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { EditDiffView } from "../tool-call/EditDiffView";
import { ReadCard } from "../tool-call/readBashCards";
import { SyntaxHighlighter } from "../utils/syntax";
import { lineDiff } from "../tool-call/diffRows";

vi.mock("../hooks/useAutoOpen", () => ({
  useAutoOpen: () => ({ shouldAutoOpen: () => true }),
}));
afterEach(cleanup);

describe("tool code syntax", () => {
  it("highlights plain Read output using its file path", () => {
    const { container } = render(<ReadCard part={{ type: "tool", tool: "Read", state: {
      status: "completed", input: { path: "main.rs" }, output: 'let name = "opman";',
    } }} />);
    expect(container.querySelector(".token.keyword")).toHaveTextContent("let");
    expect(container.querySelector(".token.string")).toHaveTextContent('"opman"');
  });
  it("keeps tokens on both sides of edits and unchanged lines", () => {
    const { container } = render(<EditDiffView input={{ file_path: "main.ts",
      old_string: 'const count = 1;\nconsole.log(count);',
      new_string: 'const count = 2;\nconsole.log(count);',
    }} />);
    expect(container.querySelector(".removed .token.number")).toHaveTextContent("1");
    expect(container.querySelector(".added .token.number")).toHaveTextContent("2");
    expect(container.querySelectorAll(".diff-line.context")).toHaveLength(1);
    expect(container.querySelector(".context .token.function")).toHaveTextContent("log");
  });
  it("keeps multiline comments across changed and unchanged lines", () => {
    const { container } = render(<EditDiffView input={{ path: "main.js",
      old_string: "/* start\nold\n*/", new_string: "/* start\nnew\n*/",
    }} />);
    expect(container.querySelector(".removed .token.comment")).toHaveTextContent("old");
    expect(container.querySelector(".added .token.comment")).toHaveTextContent("new");
  });
  it("highlights Write and preserves unknown file content safely", () => {
    const write = render(<EditDiffView input={{ path: "data.json", content: '{"count": 3}' }} />);
    expect(write.container.querySelector(".added .token.number")).toHaveTextContent("3");
    write.unmount();
    const plain = render(<EditDiffView input={{ path: "a.unknown", content: "<script>& text" }} />);
    expect(plain.container.querySelector(".added")).toHaveTextContent("<script>& text");
    expect(plain.container.querySelector("script")).toBeNull();
  });
  it("highlights each MultiEdit hunk", () => {
    const { container } = render(<EditDiffView input={{ path: "a.py", edits: [
      { old_string: "return 1", new_string: "return 2" },
      { old_string: "return 3", new_string: "return 4" },
    ] }} />);
    expect(container.querySelectorAll(".added .token.keyword")).toHaveLength(2);
  });
  it.each(["html", "cpp", "java", "ruby", "dockerfile"])("registers %s without chat mounting", language => {
    const samples: Record<string, string> = {
      html: '<div class="test">Hello</div>', cpp: "int main() { return 1; }",
      java: "class Example {}", ruby: "def hello\nend", dockerfile: "FROM ubuntu:latest",
    };
    const { container } = render(<SyntaxHighlighter language={language} useInlineStyles={false}>
      {samples[language]}
    </SyntaxHighlighter>);
    expect(container.querySelector(".token")).not.toBeNull();
  });
});

describe("bounded line diff", () => {
  it.each([
    ["", ""], ["", "one\ntwo"], ["one\ntwo", ""],
    ["start\nold\nend", "start\nnew\nend"], ["a\nb\nc", "c\nb\na"],
    [Array.from({ length: 700 }, (_, i) => `old ${i}`).join("\n"),
      Array.from({ length: 700 }, (_, i) => `new ${i}`).join("\n")],
  ])("preserves both source versions", (before, after) => {
    const rows = lineDiff(before, after);
    expect(rows.filter(row => row.type !== "add").map(row => row.text).join("\n")).toBe(before);
    expect(rows.filter(row => row.type !== "del").map(row => row.text).join("\n")).toBe(after);
  });
});
