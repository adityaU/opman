import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { osc52Text } from "../terminal-panel/osc52";
import {
  FONT_CHANGED_EVENT, MAX_FONT_SIZE, MIN_FONT_SIZE, clampFontSize, loadFontSize, stepFontSize,
} from "../terminal-panel/fontSize";
import { copyText, readText } from "../terminal-panel/clipboard";

const b64 = (text: string) => btoa(String.fromCharCode(...new TextEncoder().encode(text)));

describe("OSC 52", () => {
  it("decodes what a program asks to copy, including non-ASCII text", () => {
    expect(osc52Text(`c;${b64("hello")}`)).toBe("hello");
    expect(osc52Text(`;${b64("héllo ✓")}`)).toBe("héllo ✓");
  });

  it("refuses a read request and anything malformed", () => {
    expect(osc52Text("c;?")).toBeNull();
    expect(osc52Text("c;")).toBeNull();
    expect(osc52Text("no-separator")).toBeNull();
    expect(osc52Text("c;!!!not base64")).toBeNull();
  });
});

describe("font size", () => {
  beforeEach(() => localStorage.clear());

  it("defaults to 13 and clamps to a readable range", () => {
    expect(loadFontSize()).toBe(13);
    expect(clampFontSize(2)).toBe(MIN_FONT_SIZE);
    expect(clampFontSize(99)).toBe(MAX_FONT_SIZE);
    expect(clampFontSize(Number.NaN)).toBe(13);
  });

  it("steps, remembers, resets, and tells open terminals", () => {
    const heard: number[] = [];
    const listen = (event: Event) => heard.push((event as CustomEvent<number>).detail);
    window.addEventListener(FONT_CHANGED_EVENT, listen);
    expect(stepFontSize(1)).toBe(14);
    expect(stepFontSize(1)).toBe(15);
    expect(loadFontSize()).toBe(15);
    expect(stepFontSize(-1)).toBe(14);
    expect(stepFontSize(0)).toBe(13);
    expect(heard).toEqual([14, 15, 14, 13]);
    window.removeEventListener(FONT_CHANGED_EVENT, listen);
  });

  it("ignores a stored value that is not a number", () => {
    localStorage.setItem("opman.terminal.fontSize", "huge");
    expect(loadFontSize()).toBe(13);
  });
});

describe("clipboard", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("writes through the async clipboard when there is one", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    expect(await copyText("abc")).toBe(true);
    expect(writeText).toHaveBeenCalledWith("abc");
  });

  it("falls back to a selection copy when the async clipboard refuses", async () => {
    vi.stubGlobal("navigator", { clipboard: { writeText: vi.fn().mockRejectedValue(new Error("no")) } });
    const exec = vi.fn().mockReturnValue(true);
    Object.defineProperty(document, "execCommand", { value: exec, configurable: true });
    expect(await copyText("abc")).toBe(true);
    expect(exec).toHaveBeenCalledWith("copy");
    expect(document.querySelector("textarea")).toBeNull();
  });

  it("copies nothing for empty text, and reads null when blocked", async () => {
    expect(await copyText("")).toBe(false);
    vi.stubGlobal("navigator", { clipboard: { readText: vi.fn().mockRejectedValue(new Error("denied")) } });
    expect(await readText()).toBeNull();
    vi.stubGlobal("navigator", { clipboard: { readText: vi.fn().mockResolvedValue("pasted") } });
    expect(await readText()).toBe("pasted");
  });
});
