import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../api";
import { groupByTask, splitSessionTree } from "../sidebar/sessionTree";
import type { SessionTaskLink } from "../sidebar/useSessionTaskLinks";

const session = (id: string, updated: number, parentID = ""): SessionInfo =>
  ({ id, title: id, parentID, time: { created: 0, updated } }) as unknown as SessionInfo;

describe("splitSessionTree", () => {
  it("separates parents from children and sorts parents newest first", () => {
    const tree = splitSessionTree(
      [session("a", 1), session("b", 3), session("c", 2, "a")],
      () => false,
    );
    expect(tree.parentSessions.map((s) => s.id)).toEqual(["b", "a"]);
    expect(tree.childrenMap.get("a")?.map((s) => s.id)).toEqual(["c"]);
    expect(tree.hasActive).toBe(false);
  });

  it("is active when any session, child included, is busy", () => {
    const tree = splitSessionTree([session("a", 1), session("c", 2, "a")], (id) => id === "c");
    expect(tree.hasActive).toBe(true);
  });
});

describe("groupByTask", () => {
  const link = (taskId: string, archived = false) =>
    ({ taskId, taskTitle: `Task ${taskId}`, laneColor: "x", archived }) as unknown as SessionTaskLink;

  it("returns the input untouched without links", () => {
    const parents = [session("a", 1)];
    expect(groupByTask(parents, undefined)).toEqual({ taskGroups: [], ungroupedParents: parents });
  });

  it("groups linked sessions under their task and drops archived ones", () => {
    const parents = [session("a", 5), session("b", 4), session("c", 3), session("d", 2)];
    const links = new Map([["a", link("t1")], ["c", link("t1")], ["d", link("t2", true)]]);
    const { taskGroups, ungroupedParents } = groupByTask(parents, links);
    expect(ungroupedParents.map((s) => s.id)).toEqual(["b"]);
    expect(taskGroups).toHaveLength(1);
    expect(taskGroups[0].sessions.map((s) => s.id)).toEqual(["a", "c"]);
    expect(taskGroups[0].lastUpdated).toBe(5);
  });
});
