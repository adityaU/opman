import { describe, expect, it } from "vitest";
import {
  amendTarget,
  canStep,
  clearTarget,
  currentTarget,
  EMPTY_HISTORY,
  jumpHistory,
  peekStep,
  recentTargets,
  recordTarget,
  refreshTarget,
  repairHistory,
  sameTarget,
  stepHistory,
  targetLabel,
  type PaneHistory,
} from "../workspace/history";
import type { WidgetState } from "../workspace/types";

const ENGINE = {
  runner: "codex",
  model: null,
  agent: "",
  effort: null,
  permission: "default",
} as const;

/** Typed as the chat arm, not as the union, so a spread can add `engine`. */
const chat = (
  sessionId: string | null,
  projectPath = "/repo",
): Extract<WidgetState, { kind: "chat" }> => ({
  kind: "chat",
  projectPath,
  sessionId,
  engine: null,
});

const file = (path: string, seq = 1, projectPath = "/repo"): WidgetState => ({
  kind: "files",
  projectPath,
  sessionId: "s",
  open: { path, line: null, seq },
});

const shell = (ptyId: string | null, projectPath = "/repo"): WidgetState => ({
  kind: "terminal",
  projectPath,
  ptyId,
});

const page = (url: string | null, reveal = 0, projectPath = "/repo"): WidgetState => ({
  kind: "browser",
  projectPath,
  browserId: `proj:${projectPath}`,
  url,
  reveal,
});

const trail = (...widgets: WidgetState[]): PaneHistory =>
  widgets.reduce(recordTarget, EMPTY_HISTORY);

// ── Identity ────────────────────────────────────────────

describe("sameTarget", () => {
  it("ignores the settings of a place, not the place", () => {
    expect(sameTarget(chat("s1"), { ...chat("s1"), engine: ENGINE })).toBe(true);
    expect(sameTarget(chat("s1"), chat("s2"))).toBe(false);
  });

  it("distinguishes a file by path, not by its reveal token", () => {
    expect(sameTarget(file("a.ts", 1), file("a.ts", 9))).toBe(true);
    expect(sameTarget(file("a.ts"), file("b.ts"))).toBe(false);
  });

  it("cannot collapse the two spellings of one file, which is why callers absolutise", () => {
    // The editor panel works in project-relative paths and a reveal request
    // arrives absolute. Comparing them here would mean resolving a path against
    // a root, which is the caller's job — `onActiveFileChanged` does it, and
    // this asserts why it has to.
    expect(sameTarget(file("/repo/a.ts"), file("a.ts"))).toBe(false);
  });

  it("never treats two kinds, or two projects, as one place", () => {
    expect(sameTarget(shell("p1"), chat("p1"))).toBe(false);
    expect(sameTarget(shell("p1"), shell("p1", "/other"))).toBe(false);
  });

  it("treats a project's git panel as one place, having no target of its own", () => {
    const git = (projectPath: string): WidgetState => ({ kind: "git", projectPath });
    expect(sameTarget(git("/repo"), git("/repo"))).toBe(true);
    expect(sameTarget(git("/repo"), git("/other"))).toBe(false);
  });
});

// ── Recording ───────────────────────────────────────────

describe("recordTarget", () => {
  it("appends each new place, newest last", () => {
    const history = trail(file("a.ts"), file("b.ts"), shell("pty-1"));
    expect(history.entries).toHaveLength(3);
    expect(history.index).toBe(2);
    expect(currentTarget(history)).toEqual(shell("pty-1"));
  });

  it("replaces rather than appends when the target has not moved", () => {
    // The terminal and browser panels report on every settle, including settles
    // that did not move — unguarded, that would fill the trail with one place.
    const history = trail(page("https://x.example/"), page("https://x.example/", 4));
    expect(history.entries).toHaveLength(1);
    expect((history.entries[0] as { reveal: number }).reveal).toBe(4);
  });

  it("discards the forward tail when a step back is followed by somewhere new", () => {
    const history = trail(file("a.ts"), file("b.ts"), file("c.ts"));
    const back = stepHistory(history, -1, 2);
    expect(back).not.toBeNull();
    const branched = recordTarget(back!.history, file("d.ts"));
    expect(branched.entries.map(pathOf)).toEqual(["a.ts", "b.ts", "d.ts"]);
    expect(branched.index).toBe(2);
  });

  it("keeps the trail bounded, dropping the oldest", () => {
    let history = EMPTY_HISTORY;
    for (let n = 0; n < 40; n += 1) history = recordTarget(history, file(`f${n}.ts`));
    expect(history.entries).toHaveLength(24);
    expect(history.index).toBe(23);
    expect(pathOf(history.entries[0])).toBe("f16.ts");
    expect(pathOf(history.entries[23])).toBe("f39.ts");
  });

  it("records nothing for an empty pane", () => {
    expect(recordTarget(EMPTY_HISTORY, null)).toBe(EMPTY_HISTORY);
  });
});

describe("amendTarget", () => {
  it("updates the current entry without moving the cursor", () => {
    const history = trail(chat(null));
    const named = amendTarget(history, chat("s1"));
    expect(named.entries).toHaveLength(1);
    expect(named.index).toBe(0);
    expect(currentTarget(named)).toEqual(chat("s1"));
  });

  it("does nothing on a pane that is showing nothing", () => {
    const emptied = clearTarget(trail(file("a.ts")));
    expect(amendTarget(emptied, file("b.ts"))).toBe(emptied);
  });
});

// ── Navigating ──────────────────────────────────────────

describe("stepping", () => {
  it("walks back and forward over the same trail", () => {
    const history = trail(file("a.ts"), file("b.ts"), file("c.ts"));
    const back = stepHistory(history, -1, 10)!;
    expect(pathOf(back.widget)).toBe("b.ts");
    const further = stepHistory(back.history, -1, 11)!;
    expect(pathOf(further.widget)).toBe("a.ts");
    const forward = stepHistory(further.history, 1, 12)!;
    expect(pathOf(forward.widget)).toBe("b.ts");
  });

  it("stops at both ends", () => {
    const history = trail(file("a.ts"));
    expect(canStep(history, 1)).toBe(false);
    expect(canStep(history, -1)).toBe(false);
    expect(stepHistory(history, -1, 2)).toBeNull();
    expect(stepHistory(history, 1, 2)).toBeNull();
  });

  it("reaches the newest entry from a pane that has been emptied", () => {
    const emptied = clearTarget(trail(file("a.ts"), file("b.ts")));
    expect(currentTarget(emptied)).toBeNull();
    expect(canStep(emptied, 1)).toBe(false);
    const back = stepHistory(emptied, -1, 3)!;
    expect(pathOf(back.widget)).toBe("b.ts");
  });

  it("names where a step would land, for the menu row", () => {
    const history = trail(shell("pty-1"), file("a.ts"));
    expect(peekStep(history, -1)).toEqual(shell("pty-1"));
    expect(peekStep(history, 1)).toBeNull();
  });

  it("re-arms a file and a page so the panel acts on it again", () => {
    const history = trail(file("a.ts", 1), page("https://x.example/", 2), file("b.ts", 3));
    const backToPage = stepHistory(history, -1, 77)!;
    expect((backToPage.widget as { reveal: number }).reveal).toBe(77);

    const backToFile = stepHistory(backToPage.history, -1, 78)!;
    expect((backToFile.widget as { open: { seq: number } }).open.seq).toBe(78);
  });

  it("leaves a kind with nothing to re-arm untouched", () => {
    const attached = shell("pty-1");
    expect(refreshTarget(attached, 9)).toBe(attached);
    // A browser that has never loaded a page has nowhere to be sent.
    const blank = page(null);
    expect(refreshTarget(blank, 9)).toBe(blank);
  });

  it("writes the re-armed entry back, so the trail and the widget agree", () => {
    const history = trail(file("a.ts", 1), file("b.ts", 2));
    const back = jumpHistory(history, 0, 50)!;
    expect(back.history.entries[0]).toBe(back.widget);
    expect((back.history.entries[0] as { open: { seq: number } }).open.seq).toBe(50);
  });

  it("refuses an index that is not in the trail", () => {
    const history = trail(file("a.ts"));
    expect(jumpHistory(history, 4, 1)).toBeNull();
    expect(jumpHistory(history, -1, 1)).toBeNull();
  });
});

describe("recentTargets", () => {
  it("lists the rest newest first, leaving out where the pane is", () => {
    const history = trail(file("a.ts"), file("b.ts"), file("c.ts"));
    expect(recentTargets(history).map((entry) => pathOf(entry.widget))).toEqual(["b.ts", "a.ts"]);
  });

  it("carries the index each row jumps to", () => {
    const history = trail(file("a.ts"), file("b.ts"), file("c.ts"));
    expect(recentTargets(history).map((entry) => entry.index)).toEqual([1, 0]);
  });

  it("offers a short list rather than the whole trail", () => {
    let history = EMPTY_HISTORY;
    for (let n = 0; n < 20; n += 1) history = recordTarget(history, file(`f${n}.ts`));
    expect(recentTargets(history)).toHaveLength(8);
  });
});

// ── Labelling ───────────────────────────────────────────

describe("targetLabel", () => {
  it("names each kind from the widget alone", () => {
    expect(targetLabel(file("/repo/src/reducer.ts"))).toBe("reducer.ts");
    expect(targetLabel(page("https://docs.example.com/a/b"))).toBe("docs.example.com");
    expect(targetLabel(shell(null))).toBe("New shell");
    expect(targetLabel(chat(null))).toBe("New session");
    expect(targetLabel({ kind: "git", projectPath: "/repo" })).toBe("Git");
  });

  it("falls back to the raw value rather than throwing on a URL it cannot parse", () => {
    expect(targetLabel(page("not a url"))).toBe("not a url");
  });
});

// ── Loading ─────────────────────────────────────────────

describe("repairHistory", () => {
  it("adopts a widget the restored trail does not know about", () => {
    const repaired = repairHistory(trail(file("a.ts")), file("b.ts"));
    expect(repaired.entries.map(pathOf)).toEqual(["a.ts", "b.ts"]);
    expect(currentTarget(repaired)).toEqual(file("b.ts"));
  });

  it("keeps an agreeing trail as it is, and lets the widget win on detail", () => {
    // The same place, restored with a detail the entry does not carry — the
    // engine here, a reveal token elsewhere. One entry, and the widget's value.
    const repaired = repairHistory(trail(chat("s1")), { ...chat("s1"), engine: ENGINE });
    expect(repaired.entries).toHaveLength(1);
    expect(currentTarget(repaired)).toEqual({ ...chat("s1"), engine: ENGINE });
  });

  it("adopts a session id the restored trail disagrees with, rather than guessing", () => {
    // Only reachable from a half-finished write: `bindSession` amends, so the
    // two never drift in normal use. A different session is a different place,
    // so it is recorded rather than silently written over the old one.
    const repaired = repairHistory(trail(chat(null)), chat("s1"));
    expect(repaired.entries).toHaveLength(2);
    expect(currentTarget(repaired)).toEqual(chat("s1"));
  });

  it("parks the cursor past the end for an empty pane, keeping the trail", () => {
    const repaired = repairHistory(trail(file("a.ts")), null);
    expect(repaired.entries).toHaveLength(1);
    expect(currentTarget(repaired)).toBeNull();
    expect(canStep(repaired, -1)).toBe(true);
  });

  it("has nothing to repair when there is neither a trail nor a widget", () => {
    expect(repairHistory(EMPTY_HISTORY, null)).toBe(EMPTY_HISTORY);
  });
});

function pathOf(widget: WidgetState): string {
  return widget.kind === "files" ? widget.open?.path ?? "" : `not a file: ${widget.kind}`;
}
