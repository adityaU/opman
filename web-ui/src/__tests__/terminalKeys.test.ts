import { describe, expect, it } from "vitest";
import { keyAction, leavesToBrowser, type KeyFacts } from "../terminal-panel/keys";

const k = (key: string, mods: Partial<KeyFacts> = {}): KeyFacts => ({
  key, ctrl: false, meta: false, shift: false, alt: false, ...mods,
});

describe("copy", () => {
  it("is Ctrl+Shift+C or Ctrl+Insert on Windows and Linux, with or without a selection", () => {
    expect(keyAction(k("C", { ctrl: true, shift: true }), false, false)).toBe("copy");
    expect(keyAction(k("Insert", { ctrl: true }), false, false)).toBe("copy");
  });

  it("is plain Ctrl+C only while text is selected, so interrupting still works", () => {
    expect(keyAction(k("c", { ctrl: true }), false, true)).toBe("copy");
    expect(keyAction(k("c", { ctrl: true }), false, false)).toBeNull();
  });

  it("is Cmd+C on a Mac, and Ctrl+C there is always the interrupt", () => {
    expect(keyAction(k("c", { meta: true }), true, true)).toBe("copy");
    expect(keyAction(k("c", { meta: true }), true, false)).toBeNull();
    expect(keyAction(k("c", { ctrl: true }), true, true)).toBeNull();
  });
});

describe("paste", () => {
  it("is left to the browser's own paste event", () => {
    const paste = keyAction(k("V", { ctrl: true, shift: true }), false, false);
    expect(paste).toBe("paste");
    expect(leavesToBrowser("paste")).toBe(true);
    expect(keyAction(k("Insert", { shift: true }), false, false)).toBe("paste");
    expect(keyAction(k("v", { meta: true }), true, false)).toBe("paste");
  });

  it("does not take plain Ctrl+V, which a shell or vim may want", () => {
    expect(keyAction(k("v", { ctrl: true }), false, false)).toBeNull();
  });

  it("is the only action the browser finishes", () => {
    for (const action of ["copy", "selectAll", "zoomIn", "zoomOut", "zoomReset", "scrollTop", "scrollBottom"] as const) {
      expect(leavesToBrowser(action)).toBe(false);
    }
  });
});

describe("the rest", () => {
  it("selects all with Cmd+A or Ctrl+Shift+A, never plain Ctrl+A (line start)", () => {
    expect(keyAction(k("a", { meta: true }), true, false)).toBe("selectAll");
    expect(keyAction(k("A", { ctrl: true, shift: true }), false, false)).toBe("selectAll");
    expect(keyAction(k("a", { ctrl: true }), false, false)).toBeNull();
  });

  it("changes the font with Ctrl and plus or minus, and resets with Ctrl+Shift+0", () => {
    expect(keyAction(k("=", { ctrl: true }), false, false)).toBe("zoomIn");
    expect(keyAction(k("+", { ctrl: true, shift: true }), false, false)).toBe("zoomIn");
    expect(keyAction(k("-", { ctrl: true }), false, false)).toBe("zoomOut");
    expect(keyAction(k(")", { ctrl: true, shift: true }), false, false)).toBe("zoomReset");
    expect(keyAction(k("=", { meta: true }), true, false)).toBe("zoomIn");
  });

  it("scrolls to the ends with Shift+Home and Shift+End", () => {
    expect(keyAction(k("Home", { shift: true }), false, false)).toBe("scrollTop");
    expect(keyAction(k("End", { shift: true }), false, false)).toBe("scrollBottom");
  });

  it("leaves Alt chords and ordinary keys to the shell", () => {
    expect(keyAction(k("c", { ctrl: true, shift: true, alt: true }), false, true)).toBeNull();
    expect(keyAction(k("f", { alt: true }), false, false)).toBeNull();
    expect(keyAction(k("a"), false, true)).toBeNull();
    expect(keyAction(k("Home"), false, false)).toBeNull();
  });
});
