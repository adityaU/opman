import { describe, expect, it } from "vitest";
import { emptyWorkspace, workspaceReducer, type WorkspaceAction } from "../workspace/reducer";
import { findPane, paneIds } from "../workspace/tree";
import type { PaneId, WidgetState, Workspace } from "../workspace/types";

const chat = (sessionId: string | null, projectPath = "/repo"): WidgetState => ({
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

describe("a pane's history through the reducer", () => {
  const first = (state: Workspace) => paneIds(state.windows[0].root)[0];
  const run = (state: Workspace, ...actions: WorkspaceAction[]) =>
    actions.reduce(workspaceReducer, state);
  const paneAt = (state: Workspace, pane: PaneId) => {
    const window = state.windows.find((w) => w.id === state.activeWindowId) ?? state.windows[0];
    return findPane(window.root, pane);
  };

  const consistent = (state: Workspace) => {
    for (const window of state.windows) {
      for (const pane of paneIds(window.root).map((id) => findPane(window.root, id))) {
        if (!pane) continue;
        const { entries, index } = pane.history;
        if (pane.widget) expect(entries[index]).toBe(pane.widget);
        else expect(index).toBe(entries.length);
      }
    }
  };

  it("records a walk across widget kinds, and walks back through it", () => {
    let state = emptyWorkspace();
    const pane = first(state);
    state = run(
      state,
      { type: "openWidget", pane, widget: file("a.ts") },
      { type: "openWidget", pane, widget: file("b.ts") },
      { type: "openWidget", pane, widget: shell("pty-1") },
      { type: "openWidget", pane, widget: chat("s1") },
    );
    consistent(state);
    expect(paneAt(state, pane)?.history.entries).toHaveLength(4);

    state = run(state, { type: "historyStep", pane, step: -1, seq: 10 });
    expect(paneAt(state, pane)?.widget).toEqual(shell("pty-1"));
    state = run(state, { type: "historyStep", pane, step: -1, seq: 11 });
    expect(pathOf(paneAt(state, pane)!.widget!)).toBe("b.ts");
    consistent(state);

    state = run(state, { type: "historyStep", pane, step: 1, seq: 12 });
    expect(paneAt(state, pane)?.widget).toEqual(shell("pty-1"));
    consistent(state);
  });

  it("jumps straight to a recent entry", () => {
    let state = emptyWorkspace();
    const pane = first(state);
    state = run(
      state,
      { type: "openWidget", pane, widget: file("a.ts") },
      { type: "openWidget", pane, widget: file("b.ts") },
      { type: "openWidget", pane, widget: file("c.ts") },
      { type: "historyJump", pane, index: 0, seq: 20 },
    );
    expect(pathOf(paneAt(state, pane)!.widget!)).toBe("a.ts");
    consistent(state);
  });

  it("does not record an amend, so one conversation is one entry", () => {
    let state = emptyWorkspace();
    const pane = first(state);
    state = run(
      state,
      { type: "openWidget", pane, widget: chat(null) },
      { type: "amendWidget", pane, widget: chat("s1") },
    );
    expect(paneAt(state, pane)?.history.entries).toHaveLength(1);
    expect(paneAt(state, pane)?.widget).toEqual(chat("s1"));
    consistent(state);
  });

  it("keeps the trail when a pane is emptied, so back reopens what was there", () => {
    let state = emptyWorkspace();
    const pane = first(state);
    state = run(
      state,
      { type: "openWidget", pane, widget: file("a.ts") },
      { type: "openWidget", pane, widget: null },
    );
    expect(paneAt(state, pane)?.widget).toBeNull();
    consistent(state);

    state = run(state, { type: "historyStep", pane, step: -1, seq: 30 });
    expect(pathOf(paneAt(state, pane)!.widget!)).toBe("a.ts");
  });

  it("carries each trail with its widget when two panes are swapped", () => {
    let state = emptyWorkspace();
    const left = first(state);
    state = run(
      state,
      { type: "openWidget", pane: left, widget: file("a.ts") },
      { type: "openWidget", pane: left, widget: file("b.ts") },
      { type: "splitPane", pane: left, dir: "row", widget: shell("pty-1") },
    );
    const [one, two] = paneIds(state.windows[0].root);

    state = run(state, { type: "swapWidgets", from: one, to: two });
    consistent(state);
    expect(paneAt(state, two)?.history.entries.map(pathOf)).toEqual(["a.ts", "b.ts"]);
    expect(paneAt(state, one)?.history.entries).toHaveLength(1);
  });

  it("carries the trail into another window", () => {
    let state = emptyWorkspace();
    const pane = first(state);
    state = run(
      state,
      { type: "openWidget", pane, widget: file("a.ts") },
      { type: "openWidget", pane, widget: file("b.ts") },
      { type: "splitPane", pane, dir: "row", widget: shell("pty-1") },
    );
    const moved = paneIds(state.windows[0].root)[0];

    state = run(state, { type: "movePaneToWindow", pane: moved, window: "new" });
    expect(state.windows).toHaveLength(2);
    consistent(state);
    const landed = paneIds(state.windows[1].root)[0];
    expect(paneAt(state, landed)?.history.entries.map(pathOf)).toEqual(["a.ts", "b.ts"]);
  });

  it("gives a pane split with a widget a trail holding it", () => {
    let state = emptyWorkspace();
    const pane = first(state);
    state = run(state, { type: "splitPane", pane, dir: "row", widget: chat("s1") });
    consistent(state);
    const created = paneIds(state.windows[0].root)[1];
    expect(paneAt(state, created)?.history.entries).toEqual([chat("s1")]);
  });
});

function pathOf(widget: WidgetState): string {
  return widget.kind === "files" ? widget.open?.path ?? "" : `not a file: ${widget.kind}`;
}
