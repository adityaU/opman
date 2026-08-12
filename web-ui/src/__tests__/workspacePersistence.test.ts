/** Persistence is total: malformed state repairs the affected branch. */
import { describe, it, expect } from "vitest";
import { loadWorkspace, saveWorkspace } from "../workspace/persistence";
import { emptyWorkspace, workspaceReducer } from "../workspace/reducer";
import { paneCount, paneIds } from "../workspace/tree";
import type { Workspace } from "../workspace/types";
function store(value?: string): Pick<Storage, "getItem" | "setItem"> {
  let held = value;
  return {
    getItem: () => held ?? null,
    setItem: (_key: string, next: string) => {
      held = next;
    },
  };
}

const stored = (workspace: unknown, version = 1) =>
  store(JSON.stringify({ version, workspace }));

function populated(): Workspace {
  let state = emptyWorkspace();
  const pane = paneIds(state.windows[0].root)[0];
  state = workspaceReducer(state, {
    type: "setWidget",
    pane,
    widget: { kind: "chat", projectPath: "/repo", sessionId: "s1", engine: null },
  });
  state = workspaceReducer(state, {
    type: "splitPane",
    pane,
    dir: "row",
    widget: { kind: "git", projectPath: "/other" },
  });
  return workspaceReducer(state, { type: "toggleChrome", level: "rail" });
}

function withWidget(widget: unknown) {
  return {
    windows: [{
      id: "w1", name: "1", root: { type: "leaf", id: "p1", widget }, focusedPaneId: "p1",
    }],
    activeWindowId: "w1",
  };
}
describe("round trip", () => {
  it("restores the tree, the widgets, the focus and the chrome", () => {
    const original = populated();
    const storage = store();
    saveWorkspace(original, storage);
    expect(loadWorkspace(storage)).toEqual(original);
  });

  it("repairs version 1 editor widgets that predate their pane identity", () => {
    const files = loadWorkspace(stored(withWidget({ kind: "files", projectPath: "/repo" })));
    expect(files.windows[0].root).toMatchObject({
      widget: { kind: "files", projectPath: "/repo", sessionId: "p1" },
    });
  });
});

describe("a hostile or stale store", () => {
  it("seeds a fresh workspace when nothing is stored", () => {
    expect(loadWorkspace(store()).windows).toHaveLength(1);
  });

  it("seeds a fresh workspace on unparseable JSON", () => {
    expect(loadWorkspace(store("{not json"))).toMatchObject({ windows: expect.any(Array) });
    expect(loadWorkspace(store("{not json")).windows).toHaveLength(1);
  });

  it("refuses a version from the future rather than misreading it", () => {
    const state = loadWorkspace(stored(populated(), 99));
    expect(state.windows).toHaveLength(1);
    expect(paneCount(state.windows[0].root)).toBe(1);
  });

  it("drops a widget whose kind it does not know, keeping the pane", () => {
    const state = loadWorkspace(stored(withWidget({ kind: "hologram", projectPath: "/x" })));
    expect(state.windows[0].root).toMatchObject({ type: "leaf", widget: null });
  });

  it("drops a widget with no project rather than rendering a pane pointed nowhere", () => {
    const state = loadWorkspace(stored(withWidget({ kind: "git" })));
    expect(state.windows[0].root).toMatchObject({ widget: null });
  });

  it("collapses a split whose children are corrupt, costing the branch not the desk", () => {
    const state = loadWorkspace(
      stored({
        windows: [
          {
            id: "w1",
            name: "1",
            root: {
              type: "split",
              id: "s1",
              dir: "row",
              sizes: [0.5, 0.5],
              children: [{ type: "leaf", id: "p1", widget: null }, { garbage: true }],
            },
            focusedPaneId: "p1",
          },
        ],
        activeWindowId: "w1",
      }),
    );
    expect(state.windows[0].root).toMatchObject({ type: "leaf", id: "p1" });
  });

  it("repairs sizes that do not sum to 1", () => {
    const state = loadWorkspace(
      stored({
        windows: [
          {
            id: "w1",
            name: "1",
            root: {
              type: "split",
              id: "s1",
              dir: "row",
              sizes: [3, 1],
              children: [
                { type: "leaf", id: "p1", widget: null },
                { type: "leaf", id: "p2", widget: null },
              ],
            },
            focusedPaneId: "p1",
          },
        ],
        activeWindowId: "w1",
      }),
    );
    const root = state.windows[0].root;
    if (root.type !== "split") throw new Error("expected a split");
    expect(root.sizes[0]).toBeCloseTo(0.75, 10);
    expect(root.sizes.reduce((a, b) => a + b, 0)).toBeCloseTo(1, 10);
  });

  it("repoints a focus and a zoom that name panes which no longer exist", () => {
    const state = loadWorkspace(
      stored({
        windows: [
          {
            id: "w1",
            name: "1",
            root: { type: "leaf", id: "p1", widget: null },
            focusedPaneId: "gone",
            zoomedPaneId: "gone",
          },
        ],
        activeWindowId: "w1",
      }),
    );
    expect(state.windows[0].focusedPaneId).toBe("p1");
    expect(state.windows[0].zoomedPaneId).toBeNull();
  });

  it("repoints an active window id that names nothing", () => {
    const state = loadWorkspace(
      stored({
        windows: [
          { id: "w1", name: "1", root: { type: "leaf", id: "p1", widget: null }, focusedPaneId: "p1" },
        ],
        activeWindowId: "ghost",
      }),
    );
    expect(state.activeWindowId).toBe("w1");
  });

  it("falls back to the default chrome when it is missing or wrong-typed", () => {
    const state = loadWorkspace(
      stored({
        windows: [
          { id: "w1", name: "1", root: { type: "leaf", id: "p1", widget: null }, focusedPaneId: "p1" },
        ],
        activeWindowId: "w1",
        chrome: { rail: false, paneHeaders: "yes" },
      }),
    );
    expect(state.chrome).toEqual({ rail: false, paneHeaders: true, zen: false });
  });

  it("survives a storage that throws on write", () => {
    const throwing = {
      setItem: () => {
        throw new Error("quota");
      },
    };
    expect(() => saveWorkspace(emptyWorkspace(), throwing)).not.toThrow();
  });
});

/**
 * PTYs live in the server process and outlive a browser refresh, so a terminal
 * pane that remembers its ids re-attaches to the running shell instead of
 * spawning a fresh one and losing the scrollback.
 */
describe("terminal survival", () => {
  const withTerminal = (ptyIds: unknown) => ({
    windows: [
      {
        id: "w1",
        name: "1",
        root: {
          type: "leaf",
          id: "p1",
          widget: { kind: "terminal", projectPath: "/repo", ptyIds },
        },
        focusedPaneId: "p1",
      },
    ],
    activeWindowId: "w1",
  });

  it("round-trips the pty ids", () => {
    const state = loadWorkspace(stored(withTerminal(["pty-a", "pty-b"])));
    expect(state.windows[0].root).toMatchObject({
      widget: { kind: "terminal", ptyIds: ["pty-a", "pty-b"] },
    });
  });

  it("survives a pane written before ids were persisted", () => {
    const state = loadWorkspace(stored(withTerminal(undefined)));
    expect(state.windows[0].root).toMatchObject({ widget: { kind: "terminal", ptyIds: [] } });
  });

  it("drops non-string ids rather than handing them to the PTY layer", () => {
    const state = loadWorkspace(stored(withTerminal(["ok", 7, null, { id: "x" }])));
    expect(state.windows[0].root).toMatchObject({ widget: { ptyIds: ["ok"] } });
  });
});

describe("pane engine", () => {
  const engine = {
    runner: "codex",
    model: { providerID: "openai", modelID: "gpt-5" },
    agent: "build",
    effort: "high",
    permission: "on-request",
  };

  const withEngine = (value: unknown) =>
    stored({
      windows: [
        {
          id: "w1",
          name: "1",
          focusedPaneId: "p1",
          root: {
            type: "leaf",
            id: "p1",
            widget: { kind: "chat", projectPath: "/repo", sessionId: "s1", engine: value },
          },
        },
      ],
      activeWindowId: "w1",
    });

  const restoredEngine = (value: unknown) => {
    const pane = loadWorkspace(withEngine(value)).windows[0].root;
    if (pane.type !== "leaf" || pane.widget?.kind !== "chat") throw new Error("not a chat pane");
    return pane.widget.engine;
  };

  it("round-trips a pane's own engine", () => {
    let state = emptyWorkspace();
    const pane = paneIds(state.windows[0].root)[0];
    state = workspaceReducer(state, {
      type: "setWidget",
      pane,
      widget: { kind: "chat", projectPath: "/repo", sessionId: "s1", engine },
    });
    const storage = store();
    saveWorkspace(state, storage);

    const back = loadWorkspace(storage).windows[0].root;
    expect(back.type === "leaf" && back.widget?.kind === "chat" && back.widget.engine).toEqual(engine);
  });

  it("reads an absent engine as 'follow the shell'", () => {
    expect(restoredEngine(undefined)).toBeNull();
  });

  it("refuses an engine with no runner rather than half-restoring one", () => {
    // Without a runner there is no catalogue for the model or agent to name.
    expect(restoredEngine({ model: { providerID: "openai", modelID: "gpt-5" } })).toBeNull();
    expect(restoredEngine({ runner: "" })).toBeNull();
  });

  it("keeps the runner and defaults the rest when the stored fields are junk", () => {
    expect(restoredEngine({ runner: "claude", model: 7, agent: [], effort: {} })).toEqual({
      runner: "claude",
      model: null,
      agent: "",
      effort: null,
      permission: "default",
    });
  });
});
