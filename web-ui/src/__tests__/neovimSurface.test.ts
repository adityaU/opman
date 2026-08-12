import { describe, expect, it } from "vitest";
import { shouldAttachNeovim } from "../code-editor/surface";

describe("shouldAttachNeovim", () => {
  it("selects Neovim on desktop when the preference says Neovim", () => {
    expect(shouldAttachNeovim("desktop", "neovim", false)).toBe(true);
  });

  it("keeps CodeMirror on desktop when the preference says CodeMirror", () => {
    expect(shouldAttachNeovim("desktop", "codemirror", false)).toBe(false);
  });

  it("never changes the mobile editor surface", () => {
    expect(shouldAttachNeovim("mobile", "neovim", true)).toBe(false);
    expect(shouldAttachNeovim(undefined, "neovim", true)).toBe(false);
  });

  it("uses the responsive fallback only when no layout is supplied", () => {
    expect(shouldAttachNeovim(undefined, "neovim", false)).toBe(true);
  });

  it("does not key off the keybinding mode", () => {
    expect(shouldAttachNeovim("desktop", "codemirror", false)).toBe(false);
  });
});
