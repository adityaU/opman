/** A widget's `server` survives a save and load; its absence stays absence. */
import { describe, it, expect } from "vitest";
import { loadWorkspace, saveWorkspace } from "../workspace/persistence";
import { parseWidget } from "../workspace/parseWidget";
import { emptyWorkspace, workspaceReducer } from "../workspace/reducer";
import { paneIds, panes } from "../workspace/tree";
import { asPaneId, type WidgetState } from "../workspace/types";

function store(): Pick<Storage, "getItem" | "setItem"> {
  let held: string | null = null;
  return { getItem: () => held, setItem: (_key, next) => { held = next; } };
}

function roundTrip(widget: WidgetState): WidgetState | null {
  let state = emptyWorkspace();
  const pane = paneIds(state.windows[0].root)[0];
  state = workspaceReducer(state, { type: "openWidget", pane, widget });
  const storage = store();
  saveWorkspace(state, storage);
  return panes(loadWorkspace(storage).windows[0].root)[0].widget;
}

describe("widget server persistence", () => {
  it("round-trips the server on every kind", () => {
    const widgets: WidgetState[] = [
      { kind: "chat", projectPath: "/r", sessionId: "s", engine: null, server: "dev" },
      { kind: "files", projectPath: "/r", sessionId: "p", open: null, server: "dev" },
      { kind: "terminal", projectPath: "/r", ptyId: "t1", server: "dev" },
      { kind: "git", projectPath: "/r", server: "dev" },
      { kind: "browser", projectPath: "/r", browserId: "proj:x", url: null, reveal: 0, server: "dev" },
    ];
    for (const widget of widgets) expect(roundTrip(widget)).toEqual(widget);
  });

  it("keeps the trail's entries tagged too", () => {
    let state = emptyWorkspace();
    const pane = paneIds(state.windows[0].root)[0];
    state = workspaceReducer(state, { type: "openWidget", pane, widget: { kind: "git", projectPath: "/r", server: "dev" } });
    state = workspaceReducer(state, { type: "openWidget", pane, widget: { kind: "git", projectPath: "/r" } });
    const storage = store();
    saveWorkspace(state, storage);
    const leaf = panes(loadWorkspace(storage).windows[0].root)[0];
    expect(leaf.history.entries.map((entry) => entry.server)).toEqual(["dev", undefined]);
  });

  it("reads absent, empty or malformed servers as the own server", () => {
    const id = asPaneId("p");
    for (const server of [undefined, "", 42, null]) {
      const widget = parseWidget({ kind: "git", projectPath: "/r", server }, id);
      expect(widget).toEqual({ kind: "git", projectPath: "/r" });
      expect(widget && "server" in widget).toBe(false);
    }
  });
});
