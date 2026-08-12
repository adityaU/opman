import { describe, expect, it } from "vitest";
import { NVIM_CAPTURE_ATTRIBUTE, nvimOwnsKey } from "./capture";

describe("Neovim key capture claim", () => {
  it("recognizes marked editor descendants but leaves app-owned fields alone", () => {
    const editor = document.createElement("div");
    editor.setAttribute(NVIM_CAPTURE_ATTRIBUTE, "true");
    const content = document.createElement("span");
    const input = document.createElement("input");
    const textarea = document.createElement("textarea");
    editor.append(content, input, textarea);
    document.body.append(editor);

    expect(nvimOwnsKey(content)).toBe(true);
    expect(nvimOwnsKey(input)).toBe(false);
    expect(nvimOwnsKey(textarea)).toBe(false);
  });

  it("does not claim an unmarked editor", () => {
    const editor = document.createElement("div");
    const content = document.createElement("span");
    editor.append(content);
    document.body.append(editor);

    expect(nvimOwnsKey(content)).toBe(false);
  });
});
