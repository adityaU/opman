/**
 * The browser widget's contract with the workspace.
 *
 * A browser is **per project**, not per pane: the id is derived from the project
 * path, and derived identically in the Rust backend. That is what lets a browser
 * an agent opened for a repo be the browser the user's pane connects to, with
 * neither side told the other's id — so these tests pin the derivation, and pin
 * that a widget saved under an older pane-scoped id migrates onto it rather than
 * being stranded on a tab nothing else can reach.
 */
import { describe, it, expect } from "vitest";
import { loadWorkspace, saveWorkspace } from "../workspace/persistence";
import { emptyWorkspace, workspaceReducer } from "../workspace/reducer";
import { paneIds } from "../workspace/tree";
import { advance, toWidget, EMPTY_DRAFT } from "../workspace/opener/steps";
import { browserIdForProject } from "../api/browser";
import { WIDGET_KINDS, asPaneId } from "../workspace/types";
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

function withBrowser(url: string | null = "https://example.com/"): Workspace {
  const state = emptyWorkspace();
  const pane = paneIds(state.windows[0].root)[0];
  return workspaceReducer(state, {
    type: "setWidget",
    pane,
    widget: {
      kind: "browser",
      projectPath: "/repo",
      browserId: browserIdForProject("/repo"),
      url,
    },
  });
}

describe("browser widget", () => {
  it("is offered as a widget kind", () => {
    expect(WIDGET_KINDS).toContain("browser");
  });

  it("carries its tab id and URL across a save/load round trip", () => {
    const backing = store();
    saveWorkspace(withBrowser(), backing);
    const loaded = loadWorkspace(backing);

    const pane = loaded?.windows[0].root;
    expect(pane?.type).toBe("leaf");
    expect(pane?.type === "leaf" && pane.widget).toEqual({
      kind: "browser",
      projectPath: "/repo",
      browserId: "proj:/repo",
      url: "https://example.com/",
    });
  });

  it("restores a pane that was never navigated", () => {
    const backing = store();
    saveWorkspace(withBrowser(null), backing);
    const pane = loadWorkspace(backing)?.windows[0].root;
    expect(pane?.type === "leaf" && pane.widget?.kind).toBe("browser");
    expect(pane?.type === "leaf" && pane.widget?.kind === "browser" && pane.widget.url).toBeNull();
  });

  it("migrates a widget saved without a project-scoped id", () => {
    // Older saved workspaces predate the per-project id. Dropping those panes on
    // load would lose the user's layout; keeping a pane-scoped id would strand
    // them on a tab the agent's tools cannot name.
    const backing = store(
      JSON.stringify({
        version: 1,
        workspace: {
          windows: [
            {
              id: "w1",
              name: "1",
              root: {
                type: "leaf",
                id: "p1",
                widget: { kind: "browser", projectPath: "/repo" },
              },
              focusedPaneId: "p1",
            },
          ],
          activeWindowId: "w1",
          chrome: { rail: true, sidebar: true, headers: true },
        },
      }),
    );
    const pane = loadWorkspace(backing)?.windows[0].root;
    expect(pane?.type === "leaf" && pane.widget?.kind === "browser" && pane.widget.browserId).toBe(
      "proj:/repo",
    );
  });

  it("needs a project but not a session in the opener", () => {
    const draft = advance(advance(EMPTY_DRAFT, "browser"), "/repo");
    expect(toWidget(draft, asPaneId("p9"))).toEqual({
      kind: "browser",
      projectPath: "/repo",
      browserId: "proj:/repo",
      url: null,
    });
  });

  it("needs no pane at all — the project alone names the browser", () => {
    const draft = advance(advance(EMPTY_DRAFT, "browser"), "/repo");
    expect(toWidget(draft)).toEqual(toWidget(draft, asPaneId("p9")));
  });

  it("two panes on one project address the same browser", () => {
    // The point of per-project ids: a second browser pane for a repo reconnects
    // to the tab already running there instead of starting a rival one.
    expect(browserIdForProject("/repo")).toBe(browserIdForProject("/repo"));
    expect(browserIdForProject("/repo")).not.toBe(browserIdForProject("/other"));
  });
});
