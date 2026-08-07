import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useGitSelection } from "../git-panel/useGitSelection";
import { useBoardSelection } from "../kanban/useBoardSelection";
import type { GitFileEntry } from "../git-panel/types";
import type { Lane, Task } from "../api/kanban";

const file = (path: string): GitFileEntry => ({ path, status: "M" }) as GitFileEntry;

const task = (id: string, laneId: string, order: number): Task =>
  ({ id, lane_id: laneId, order_index: order, title: id }) as Task;

const lane = (id: string): Lane => ({ id, name: id }) as Lane;

describe("useGitSelection", () => {
  const render = (staged: string[], unstaged: string[], untracked: string[] = []) =>
    renderHook(
      ({ s, u, n }) => useGitSelection(s.map(file), u.map(file), n.map(file)),
      { initialProps: { s: staged, u: unstaged, n: untracked } },
    );

  it("selects the first entry once files arrive", () => {
    const { result } = render(["a.ts"], ["b.ts"]);
    expect(result.current.selected).toEqual({ path: "a.ts", variant: "staged" });
  });

  it("walks across section boundaries", () => {
    const { result } = render(["a.ts"], ["b.ts"], ["c.ts"]);

    act(() => result.current.moveDown());
    expect(result.current.selected).toEqual({ path: "b.ts", variant: "unstaged" });

    act(() => result.current.moveDown());
    expect(result.current.selected).toEqual({ path: "c.ts", variant: "untracked" });
  });

  it("clamps at both ends rather than wrapping", () => {
    const { result } = render(["a.ts"], ["b.ts"]);

    act(() => result.current.moveUp());
    expect(result.current.selected?.path).toBe("a.ts");

    act(() => result.current.moveDown());
    act(() => result.current.moveDown());
    expect(result.current.selected?.path).toBe("b.ts");
  });

  it("follows a file that moves between sections", () => {
    const { result, rerender } = render([], ["b.ts", "c.ts"]);
    act(() => result.current.moveDown());
    expect(result.current.selected).toEqual({ path: "c.ts", variant: "unstaged" });

    // c.ts is staged: same path, different section.
    rerender({ s: ["c.ts"], u: ["b.ts"], n: [] });
    expect(result.current.selected).toEqual({ path: "c.ts", variant: "staged" });
  });

  it("recovers when the selected file disappears", () => {
    const { result, rerender } = render([], ["b.ts", "c.ts"]);
    act(() => result.current.moveDown());

    rerender({ s: [], u: ["b.ts"], n: [] });
    expect(result.current.selected).toEqual({ path: "b.ts", variant: "unstaged" });
  });

  it("clears when nothing is left", () => {
    const { result, rerender } = render([], ["b.ts"]);
    rerender({ s: [], u: [], n: [] });
    expect(result.current.selected).toBeUndefined();
  });

  it("reports which row is selected", () => {
    const { result } = render(["a.ts"], ["a.ts"]);
    // The same path in two sections: only the staged row is current.
    expect(result.current.isSelected("a.ts", "staged")).toBe(true);
    expect(result.current.isSelected("a.ts", "unstaged")).toBe(false);
  });
});

describe("useBoardSelection", () => {
  const lanes = [lane("todo"), lane("doing"), lane("done")];

  const byLane = (map: Record<string, Task[]>) =>
    new Map(Object.entries(map)) as ReadonlyMap<string, Task[]>;

  const render = (map: Record<string, Task[]>) =>
    renderHook(({ m }) => useBoardSelection(lanes, byLane(m)), { initialProps: { m: map } });

  const full = {
    todo: [task("t1", "todo", 1), task("t2", "todo", 2), task("t3", "todo", 3)],
    doing: [task("d1", "doing", 1)],
    done: [task("x1", "done", 1), task("x2", "done", 2)],
  };

  it("selects the first card of the first populated lane", () => {
    const { result } = render(full);
    expect(result.current.selectedId).toBe("t1");
  });

  it("walks down and up within a lane", () => {
    const { result } = render(full);
    act(() => result.current.moveDown());
    expect(result.current.selectedId).toBe("t2");
    act(() => result.current.moveUp());
    expect(result.current.selectedId).toBe("t1");
  });

  it("keeps the row position when crossing lanes", () => {
    const { result } = render({
      todo: full.todo,
      doing: [task("d1", "doing", 1), task("d2", "doing", 2), task("d3", "doing", 3)],
      done: full.done,
    });
    act(() => result.current.moveDown());
    act(() => result.current.moveDown());
    expect(result.current.selectedId).toBe("t3");

    act(() => result.current.moveRight());
    expect(result.current.selectedId).toBe("d3");
  });

  it("clamps to the last card when the next lane is shorter", () => {
    const { result } = render(full);
    act(() => result.current.moveDown());
    act(() => result.current.moveDown());
    act(() => result.current.moveRight());
    expect(result.current.selectedId).toBe("d1");
  });

  it("skips empty lanes when crossing", () => {
    const { result } = render({ todo: full.todo, doing: [], done: full.done });
    act(() => result.current.moveRight());
    expect(result.current.selectedId).toBe("x1");
  });

  it("stays put at the edges", () => {
    const { result } = render(full);
    act(() => result.current.moveLeft());
    expect(result.current.selectedId).toBe("t1");

    act(() => result.current.moveRight());
    act(() => result.current.moveRight());
    act(() => result.current.moveRight());
    expect(result.current.selectedId).toBe("x1");
  });

  it("recovers when the selected card is archived away", () => {
    const { result, rerender } = render(full);
    act(() => result.current.moveDown());
    expect(result.current.selectedId).toBe("t2");

    rerender({ m: { ...full, todo: [full.todo[0], full.todo[2]] } });
    expect(result.current.selectedId).toBe("t1");
  });

  it("clears on an empty board", () => {
    const { result } = render({ todo: [], doing: [], done: [] });
    expect(result.current.selectedId).toBeUndefined();
    act(() => result.current.moveDown());
    expect(result.current.selectedId).toBeUndefined();
  });

  it("exposes the selected task, not just its id", () => {
    const { result } = render(full);
    act(() => result.current.select("d1"));
    expect(result.current.selectedTask?.lane_id).toBe("doing");
  });
});
