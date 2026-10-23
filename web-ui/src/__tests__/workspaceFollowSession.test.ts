/**
 * A conversation that moved to a new session (a runner handoff) takes its panes with it —
 * in every window — and Back returns each pane to the session it left.
 */
import { describe, expect, it } from "vitest";
import { emptyWorkspace, workspaceReducer, type WorkspaceAction } from "../workspace/reducer";
import { findPane, paneIds } from "../workspace/tree";
import type { WidgetState, Workspace } from "../workspace/types";

const chat = (sessionId: string | null): WidgetState => ({
  kind: "chat",
  projectPath: "/repo",
  sessionId,
  engine: null,
});

const run = (state: Workspace, ...actions: WorkspaceAction[]) => actions.reduce(workspaceReducer, state);

function sessionsOf(state: Workspace): (string | null)[] {
  return state.windows.flatMap((window) =>
    paneIds(window.root).map((id) => {
      const widget = findPane(window.root, id)?.widget;
      return widget?.kind === "chat" ? widget.sessionId : null;
    }),
  );
}

describe("followSession", () => {
  it("moves every pane on the old session, in every window, and leaves the rest", () => {
    const start = emptyWorkspace();
    const firstPane = paneIds(start.windows[0].root)[0];
    let state = run(
      start,
      { type: "openWidget", pane: firstPane, widget: chat("old") },
      { type: "splitPane", pane: firstPane, dir: "row", widget: chat("other") },
      { type: "newWindow", widget: chat("old") },
    );
    state = run(state, { type: "followSession", from: "old", to: "new" });
    expect(sessionsOf(state).sort()).toEqual(["new", "new", "other"]);
  });

  it("records the move, so Back returns to the old session", () => {
    const start = emptyWorkspace();
    const pane = paneIds(start.windows[0].root)[0];
    let state = run(start, { type: "openWidget", pane, widget: chat("old") });
    state = run(state, { type: "followSession", from: "old", to: "new" });
    state = run(state, { type: "historyStep", pane, step: -1, seq: 1 });
    expect(sessionsOf(state)).toEqual(["old"]);
  });

  it("reads the target session's engine after a handoff instead of keeping the source", () => {
    const start = emptyWorkspace();
    const pane = paneIds(start.windows[0].root)[0];
    const widget: WidgetState = {
      kind: "chat", projectPath: "/repo", sessionId: "old",
      engine: { runner: "claude", model: { providerID: "anthropic", modelID: "opus" },
        agent: "", effort: null, permission: "default" },
    };
    const state = run(start, { type: "openWidget", pane, widget },
      { type: "followSession", from: "old", to: "new" });
    const target = findPane(state.windows[0].root, pane)?.widget;
    expect(target?.kind === "chat" ? target.engine : "missing").toBeNull();
  });

  it("returns the same state when no pane shows the session", () => {
    const start = emptyWorkspace();
    expect(workspaceReducer(start, { type: "followSession", from: "a", to: "b" })).toBe(start);
  });
});
