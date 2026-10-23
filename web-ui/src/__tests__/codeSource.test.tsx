import React from "react";
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SyntaxHighlighterProps } from "react-syntax-highlighter";
import { CodeSource } from "../message-turn/CodeSource";
import { SyntaxHighlighter } from "../utils/syntax";

vi.mock("../utils/syntax", async () => {
  const actual = await vi.importActual<typeof import("../utils/syntax")>("../utils/syntax");
  return { ...actual, SyntaxHighlighter: vi.fn((props: SyntaxHighlighterProps) =>
    React.createElement(actual.SyntaxHighlighter, props)) };
});

afterEach(() => {
  cleanup();
  vi.mocked(SyntaxHighlighter).mockClear();
});

function sourceText(container: HTMLElement): string {
  return [...container.querySelectorAll(".code-source-text")].map((line) => line.textContent).join("");
}

describe("CodeSource", () => {
  it("keeps multiline comment state across source lines and preserves exact source text", () => {
    const code = "/* first line\nstill a comment\n*/\nconst value = 7;";
    const { container } = render(<CodeSource language="javascript" code={code} wordWrap />);
    const second = container.querySelector('[data-source-line="2"]');
    expect(second?.querySelector(".token.comment")?.textContent).toContain("still a comment");
    expect(container.querySelector(".token.keyword")?.textContent).toBe("const");
    expect(sourceText(container)).toBe(code);
  });

  it("pairs each source line with its own gutter, including empty and trailing lines", () => {
    const { container } = render(<CodeSource language="text" code={"one\n\nthree\n"} wordWrap />);
    const rows = [...container.querySelectorAll(".code-source-row")];
    expect(rows).toHaveLength(4);
    expect(rows.map((row) => row.querySelector(".code-source-number")?.textContent)).toEqual(["1", "2", "3", "4"]);
    expect(rows.every((row) => row.querySelector(".code-source-number")?.getAttribute("aria-hidden") === "true")).toBe(true);
    expect(sourceText(container)).toBe("one\n\nthree\n");
  });

  it("switches between wrapping and scrolling without highlighting the snippet again", () => {
    const code = `const url = "${"x".repeat(2_000)}";`;
    const { container, rerender } = render(<CodeSource language="javascript" code={code} wordWrap />);
    expect(container.firstElementChild?.classList.contains("code-source--wrap")).toBe(true);
    expect(container.querySelector("code")?.style.whiteSpace).toBe("inherit");
    expect(vi.mocked(SyntaxHighlighter)).toHaveBeenCalledTimes(1);
    rerender(<CodeSource language="javascript" code={code} wordWrap={false} />);
    expect(container.firstElementChild?.classList.contains("code-source--nowrap")).toBe(true);
    expect(vi.mocked(SyntaxHighlighter)).toHaveBeenCalledTimes(1);
    expect(sourceText(container)).toBe(code);
  });

  it("widens the shared gutter for larger line counts", () => {
    const code = Array.from({ length: 101 }, (_, index) => `line ${index + 1}`).join("\n");
    const { container } = render(<CodeSource language="text" code={code} wordWrap />);
    expect(container.querySelector<HTMLElement>(".code-source-rows")?.style.getPropertyValue("--code-source-digits")).toBe("3");
    expect(sourceText(container)).toBe(code);
  });

  it("shows one numbered empty source line for an empty file", () => {
    const { container } = render(<CodeSource language="text" code="" wordWrap />);
    expect(container.querySelectorAll(".code-source-row")).toHaveLength(1);
    expect(container.querySelector(".code-source-number")?.textContent).toBe("1");
    expect(sourceText(container)).toBe("");
  });
});
