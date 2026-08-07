import { describe, expect, it } from "vitest";
import { parseChord } from "../keybindings/chord";
import { commandLabel, findCommand } from "../keybindings/commands";
import { builtInLayers } from "../keybindings/layers";
import { buildHints, Keymap, type MatchContext } from "../keybindings/matcher";
import { resolve } from "../keybindings/resolve";
import type { Host, Mode, ResolvedBinding } from "../keybindings/types";

const HOST: Host = { platform: "linux", target: "web", browser: "chrome" };

function keymapFor(mode: Mode): Keymap {
  return new Keymap(resolve(builtInLayers(), { host: HOST, mode }).bindings);
}

/**
 * The default context is "a session is open and nothing in particular has
 * focus" — every `focus==` clause false. A context where everything is true is
 * not a state the app can be in, and would make the specificity rule untestable.
 */
function context(overrides: Partial<MatchContext> = {}): MatchContext {
  return {
    mode: "normal",
    textInput: false,
    isTrue: (clause) => !clause.startsWith("focus=="),
    ...overrides,
  };
}

/** Feed a chord one step at a time, returning the final result. */
function press(keymap: Keymap, chord: string, ctx: MatchContext) {
  let pending = [...[]] as ReturnType<typeof parseChord>[number][];
  let result = keymap.match([], parseChord(chord)[0], ctx);
  for (const step of parseChord(chord)) {
    result = keymap.match(pending, step, ctx);
    if (result.type !== "pending") return result;
    pending = [...result.steps];
  }
  return result;
}

const labelFor = (b: ResolvedBinding) =>
  b.label ?? (findCommand(b.command) ? commandLabel(findCommand(b.command)!) : b.command);

describe("dispatch", () => {
  const normal = keymapFor("normal");

  it("runs a single-step chord", () => {
    expect(press(normal, "ctrl+b", context())).toMatchObject({
      type: "run",
      command: "layout.toggleSidebar",
    });
  });

  it("waits for the second step of a chord", () => {
    const first = normal.match([], parseChord("ctrl+k")[0], context());
    expect(first.type).toBe("pending");
  });

  it("runs the second step of a chord", () => {
    expect(press(normal, "ctrl+k ctrl+c", context())).toMatchObject({
      type: "run",
      command: "chat.compact",
    });
  });

  it("falls through on an unbound chord", () => {
    expect(press(normal, "ctrl+alt+shift+f9", context())).toEqual({ type: "none" });
  });

  it("skips a binding whose when clause is false", () => {
    const ctx = context({ isTrue: (clause) => clause !== "sessionActive" });
    expect(press(normal, "ctrl+k ctrl+c", ctx)).toEqual({ type: "none" });
  });

  it("prefers the scoped binding over the unscoped one", () => {
    const ctx = context({ isTrue: (clause) => clause === "focus==document" });
    expect(press(normal, "ctrl+b", ctx)).toMatchObject({ command: "doc.bold" });
  });
});

describe("insert guard", () => {
  const vim = keymapFor("vim");
  const typing = context({ mode: "vim", textInput: true });

  it("lets a bare key type instead of dispatching", () => {
    expect(press(vim, "a", typing)).toEqual({ type: "none" });
    expect(press(vim, "a", context({ mode: "vim" }))).toMatchObject({ type: "run" });
  });

  it("does not start a leader chord while typing", () => {
    expect(vim.match([], parseChord("<space>")[0], typing)).toEqual({ type: "none" });
  });

  it("still dispatches modifier chords while typing", () => {
    expect(press(vim, "ctrl+b", typing)).toMatchObject({ command: "layout.toggleSidebar" });
  });

  it("still dispatches Escape while typing", () => {
    expect(press(vim, "escape", typing).type).toBe("run");
  });

  it("keeps composer bindings alive while typing", () => {
    const ctx = context({ mode: "vim", textInput: true, isTrue: (c) => c === "composerFocused" });
    expect(press(vim, "enter", ctx)).toMatchObject({ command: "chat.send" });
  });
});

describe("vim leader", () => {
  const vim = keymapFor("vim");
  const ctx = context({ mode: "vim" });

  it("resolves a leader chord", () => {
    expect(press(vim, "<space>gg", ctx)).toMatchObject({ command: "layout.toggleGit" });
  });

  it("treats a namespace as pending, never as a command", () => {
    const afterLeader = vim.match([], parseChord("<space>")[0], ctx);
    expect(afterLeader.type).toBe("pending");
    const afterG = vim.match(parseChord("<space>"), parseChord("g")[0], ctx);
    expect(afterG.type).toBe("pending");
  });

  it("resolves a bracket motion", () => {
    expect(press(vim, "]s", ctx)).toMatchObject({ command: "session.next" });
  });
});

describe("which-key hints", () => {
  const vim = keymapFor("vim");
  const ctx = context({ mode: "vim" });

  it("collapses each namespace into one row", () => {
    const hints = buildHints(vim, parseChord("<space>"), ctx, labelFor);
    const rows = hints.flatMap((g) => g.entries);
    const git = rows.find((r) => r.key.key === "g");
    expect(git).toMatchObject({ isPrefix: true, label: "+git" });
    expect(rows.filter((r) => r.key.key === "g")).toHaveLength(1);
  });

  it("lists the leaves of a namespace with their labels", () => {
    const hints = buildHints(vim, parseChord("<space>g"), ctx, labelFor);
    const entries = hints.flatMap((g) => g.entries);
    expect(entries.find((e) => e.key.key === "b")).toMatchObject({
      label: "branch",
      isPrefix: false,
    });
    expect(entries.every((e) => e.label.length > 0)).toBe(true);
  });

  it("hides continuations whose when clause is false", () => {
    const gated = context({ mode: "vim", isTrue: (clause) => clause !== "gitRepo" });
    const entries = buildHints(vim, parseChord("<space>g"), gated, labelFor).flatMap(
      (g) => g.entries,
    );
    expect(entries.map((e) => e.key.key)).not.toContain("b");
  });

  it("offers the top-level namespaces after the leader alone", () => {
    const groups = buildHints(vim, parseChord("<space>"), ctx, labelFor).map((g) => g.group);
    expect(groups).toEqual(expect.arrayContaining(["git", "sessions", "chat", "window"]));
  });
});
