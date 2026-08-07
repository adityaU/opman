import { describe, it, expect } from "vitest";
import {
  applyModifiers, nextModifierState, ARROW_KEYS, EXTRA_KEYS,
} from "../terminal-panel/mobile/keys";
import { encodeForPty } from "../terminal-panel/encode";

describe("applyModifiers", () => {
  it("passes input through when nothing is held", () => {
    expect(applyModifiers("c", false, false)).toBe("c");
  });

  it("maps a letter to its control code", () => {
    expect(applyModifiers("c", true, false)).toBe("\x03");
    expect(applyModifiers("C", true, false)).toBe("\x03");
    expect(applyModifiers("d", true, false)).toBe("\x04");
  });

  it("sends ESC before the key for Alt", () => {
    expect(applyModifiers("b", false, true)).toBe("\x1bb");
  });

  it("combines Ctrl and Alt", () => {
    expect(applyModifiers("c", true, true)).toBe("\x1b\x03");
  });

  it("leaves sequences Ctrl cannot encode alone", () => {
    // An escape sequence is already multiple bytes — mangling it would send
    // garbage rather than an arrow key.
    expect(applyModifiers("\x1b[A", true, false)).toBe("\x1b[A");
  });

  it("maps Ctrl+Space to NUL", () => {
    expect(applyModifiers(" ", true, false)).toBe("\x00");
  });
});

describe("nextModifierState", () => {
  it("cycles off → armed → locked → off", () => {
    expect(nextModifierState("off")).toBe("armed");
    expect(nextModifierState("armed")).toBe("locked");
    expect(nextModifierState("locked")).toBe("off");
  });
});

describe("key table", () => {
  it("sends real arrow escape sequences", () => {
    expect(ARROW_KEYS.map((k) => k.seq)).toEqual([
      "\x1b[D", "\x1b[A", "\x1b[B", "\x1b[C",
    ]);
  });

  it("gives every key a distinct id and a title for assistive tech", () => {
    const all = [...ARROW_KEYS, ...EXTRA_KEYS];
    expect(new Set(all.map((k) => k.id)).size).toBe(all.length);
    expect(all.every((k) => k.title.length > 0)).toBe(true);
  });
});

describe("encodeForPty", () => {
  it("base64-encodes UTF-8 bytes", () => {
    expect(encodeForPty("\x03")).toBe(btoa("\x03"));
    expect(encodeForPty("é")).toBe(btoa("\xc3\xa9"));
  });
});
