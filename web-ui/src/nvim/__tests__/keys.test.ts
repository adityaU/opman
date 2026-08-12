import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { KEY_TABLE, KEYPAD_TABLE, KEY_TABLE_NOTATIONS } from "../input/keyTable";
import { dispatchKeyDown, encodeKey } from "../input/keys";

function keyEvent(key: string, init: KeyboardEventInit = {}, code = ""): KeyboardEvent {
  const event = new KeyboardEvent("keydown", { key, code, ...init });
  return event;
}

function altGraphEvent(key: string, init: KeyboardEventInit = {}, code = ""): KeyboardEvent {
  const event = keyEvent(key, init, code);
  Object.defineProperty(event, "getModifierState", { value: (name: string) => name === "AltGraph" });
  return event;
}

function keyFixture(): string[] {
  const value: unknown = JSON.parse(readFileSync(`${process.cwd()}/src/nvim/__fixtures__/keys.json`, "utf8"));
  if (!Array.isArray(value) || !value.every((entry): entry is string => typeof entry === "string")) {
    throw new Error("keys.json must be a plain JSON array of notation strings");
  }
  return value;
}

describe("Neovim key encoding", () => {
  it("exports every named table entry as angle-bracket notation", () => {
    expect(KEY_TABLE_NOTATIONS).toContain("F37");
    for (const notation of KEY_TABLE_NOTATIONS) {
      const key = Object.entries(KEY_TABLE).find(([, value]) => value === notation)?.[0] ?? "Enter";
      const code = Object.entries(KEYPAD_TABLE).find(([, value]) => value === notation)?.[0] ?? "";
      expect(encodeKey(keyEvent(key, {}, code))).toBe(`<${notation}>`);
    }
    expect(keyFixture()).toEqual(KEY_TABLE_NOTATIONS.map((notation) => `<${notation}>`));
  });

  it("escapes literal angle brackets and backslashes in wrappers", () => {
    expect(encodeKey(keyEvent("<"))).toBe("<lt>");
    expect(encodeKey(keyEvent("<", { ctrlKey: true }))).toBe("<C-lt>");
    expect(encodeKey(keyEvent("\\", { ctrlKey: true }))).toBe("<C-Bslash>");
  });

  it("uses named key names before composing modifiers", () => {
    expect(encodeKey(keyEvent("ArrowLeft", { ctrlKey: true }, "ArrowLeft"))).toBe("<C-Left>");
    expect(encodeKey(keyEvent("Enter", { ctrlKey: true }))).toBe("<C-CR>");
    expect(encodeKey(keyEvent("F5", { shiftKey: true }))).toBe("<S-F5>");
    expect(encodeKey(keyEvent("Tab", { shiftKey: true, ctrlKey: true }))).toBe("<S-C-Tab>");
  });

  it("composes Shift, Control, Alt, and Meta in Neovim order", () => {
    expect(encodeKey(keyEvent("F5", { shiftKey: true, ctrlKey: true, altKey: true, metaKey: true })))
      .toBe("<S-C-A-D-F5>");
  });

  it("does not add Shift to printable keys", () => {
    expect(encodeKey(keyEvent("A", { shiftKey: true }))).toBe("A");
    expect(encodeKey(keyEvent("!", { shiftKey: true }))).toBe("!");
  });

  it("recovers keypad and Ctrl punctuation bases from code", () => {
    expect(encodeKey(keyEvent("Enter", {}, "NumpadEnter"))).toBe("<kEnter>");
    expect(encodeKey(keyEvent("+", {}, "NumpadAdd"))).toBe("<kPlus>");
    expect(encodeKey(keyEvent("@", { ctrlKey: true }, "Digit2"))).toBe("<C-2>");
    expect(encodeKey(keyEvent("?", { ctrlKey: true }, "Slash"))).toBe("<C-/> ".trim());
  });

  it("suppresses AltGraph modifiers and supports macOS Option policy", () => {
    expect(encodeKey(altGraphEvent("@", { altKey: true, ctrlKey: true }, "Digit2"))).toBe("@");
    expect(encodeKey(keyEvent("å", { altKey: true }, "KeyA"))).toBe("<A-a>");
    expect(encodeKey(keyEvent("å", { altKey: true }, "KeyA"), { macAltIsMeta: true })).toBe("<D-a>");
  });

  it("ignores IME and modifier-only keydowns and does not emit unknown names", () => {
    expect(encodeKey(keyEvent("Control", { ctrlKey: true }))).toBeNull();
    expect(encodeKey(keyEvent("Shift", { shiftKey: true }))).toBeNull();
    expect(encodeKey(keyEvent("Alt", { altKey: true }))).toBeNull();
    expect(encodeKey(keyEvent("Meta", { metaKey: true }))).toBeNull();
    expect(encodeKey(keyEvent("x", { isComposing: true }))).toBeNull();
    expect(encodeKey(keyEvent("x", { keyCode: 229 }))).toBeNull();
    expect(encodeKey(keyEvent("F38"))).toBeNull();
  });

  it("prevents consumed keys while exposing a release-focus chord", () => {
    const onInput = vi.fn();
    const onReleaseFocus = vi.fn();
    const release = keyEvent("Escape", { ctrlKey: true });
    const prevent = vi.spyOn(release, "preventDefault");
    expect(dispatchKeyDown(release, {
      onInput,
      onReleaseFocus,
      releaseFocus: { key: "Escape", ctrlKey: true },
    })).toBe(true);
    expect(prevent).toHaveBeenCalledOnce();
    expect(onReleaseFocus).toHaveBeenCalledOnce();
    expect(onInput).not.toHaveBeenCalled();
  });
});
