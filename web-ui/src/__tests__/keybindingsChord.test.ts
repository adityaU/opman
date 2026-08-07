import { describe, expect, it } from "vitest";
import {
  ChordParseError,
  displayChord,
  expandLeaders,
  formatChord,
  isPrefixOf,
  parseChord,
  stepFromEvent,
} from "../keybindings/chord";

const chord = (s: string) => formatChord(parseChord(s));

describe("parseChord — modifier chords", () => {
  it("normalizes modifier order", () => {
    expect(chord("shift+ctrl+p")).toBe("ctrl+shift+p");
    expect(chord("meta+alt+shift+n")).toBe("shift+alt+meta+n");
  });

  it("splits a sequence on whitespace", () => {
    expect(parseChord("ctrl+k ctrl+w")).toHaveLength(2);
    expect(chord("ctrl+k ctrl+w")).toBe("ctrl+k ctrl+w");
  });

  it("accepts a literal plus as the key", () => {
    expect(chord("ctrl++")).toBe("ctrl++");
  });

  it("folds named keys", () => {
    expect(chord("<esc>")).toBe("escape");
    expect(chord("ctrl+<cr>")).toBe("ctrl+enter");
  });

  it("rejects unknown modifiers and unclosed angle keys", () => {
    expect(() => parseChord("hyper+p")).toThrow(ChordParseError);
    expect(() => parseChord("<leader")).toThrow(ChordParseError);
    expect(() => parseChord("<nope>")).toThrow(ChordParseError);
    expect(() => parseChord("   ")).toThrow(ChordParseError);
  });
});

describe("parseChord — vim character runs", () => {
  it("expands each character into its own step", () => {
    expect(chord("]s")).toBe("] s");
    expect(parseChord("gcc")).toHaveLength(3);
  });

  it("reads an upper-case letter as the shifted key", () => {
    expect(chord("A")).toBe("shift+a");
    expect(chord("zM")).toBe("z shift+m");
  });

  it("mixes angle keys into a run", () => {
    expect(chord("<leader>sn")).toBe("<leader> s n");
  });
});

describe("expandLeaders", () => {
  const leader = parseChord("<space>");
  const local = parseChord(",");

  it("substitutes the configured leader", () => {
    const seq = expandLeaders(parseChord("<leader>gg"), leader, local);
    expect(formatChord(seq)).toBe("space g g");
  });

  it("supports a multi-step leader", () => {
    const seq = expandLeaders(parseChord("<leader>a"), parseChord("ctrl+w"), local);
    expect(formatChord(seq)).toBe("ctrl+w a");
  });

  it("leaves a leaderless chord untouched", () => {
    const input = parseChord("ctrl+b");
    expect(expandLeaders(input, leader, local)).toBe(input);
  });
});

describe("isPrefixOf", () => {
  it("matches a strict prefix only", () => {
    expect(isPrefixOf(parseChord("space g"), parseChord("space g g"))).toBe(true);
    expect(isPrefixOf(parseChord("space g"), parseChord("space g"))).toBe(false);
    expect(isPrefixOf(parseChord("space g"), parseChord("space f f"))).toBe(false);
  });
});

describe("stepFromEvent", () => {
  const event = (init: Partial<KeyboardEvent>) => init as KeyboardEvent;

  it("lower-cases printable keys and keeps shift", () => {
    const step = stepFromEvent(event({ key: "P", shiftKey: true, ctrlKey: true }));
    expect(step).toMatchObject({ key: "p", shift: true, ctrl: true });
  });

  it("folds arrow and space keys to our names", () => {
    expect(stepFromEvent(event({ key: "ArrowDown" })).key).toBe("down");
    expect(stepFromEvent(event({ key: " " })).key).toBe("space");
  });
});

describe("displayChord", () => {
  it("uses symbols on mac and words elsewhere", () => {
    expect(displayChord(parseChord("shift+meta+p"), "mac")).toBe("⇧⌘P");
    expect(displayChord(parseChord("ctrl+shift+p"), "win")).toBe("Ctrl+Shift+P");
  });

  it("joins the steps of a sequence with a space", () => {
    expect(displayChord(parseChord("ctrl+k ctrl+w"), "win")).toBe("Ctrl+K Ctrl+W");
  });
});
