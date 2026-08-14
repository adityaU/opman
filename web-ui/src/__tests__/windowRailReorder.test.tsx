/**
 * Reordering windows.
 *
 * Two halves: the reducer's splice, where the interesting cases are the drops
 * that must not move anything, and the rail's gesture, where what matters is
 * that a drag over the bottom half of a chip lands after it and one over the top
 * half lands before it. The order itself needs no test for persistence — it is
 * the `windows` array, which the layout already saves whole.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { WindowRail } from "../workspace/WindowRail";
import { emptyWindow, workspaceReducer } from "../workspace/reducer";
import { asPaneId, asWindowId, DEFAULT_CHROME, type WindowId, type Workspace, type WorkspaceWindow } from "../workspace/types";
import { EMPTY_HISTORY } from "../workspace/history";

const makeWindow = (name: string): WorkspaceWindow => ({
  id: asWindowId(`w-${name}`),
  name,
  focusedPaneId: asPaneId(`p-${name}`),
  zoomedPaneId: null,
  root: { type: "leaf", id: asPaneId(`p-${name}`), widget: null, history: EMPTY_HISTORY },
});

const names = (workspace: Workspace) => workspace.windows.map((w) => w.name);

const workspace = (...windows: readonly WorkspaceWindow[]): Workspace => ({
  windows: [...windows],
  activeWindowId: windows[0].id,
  chrome: DEFAULT_CHROME,
});

const renderRail = (
  windows: readonly WorkspaceWindow[],
  onReorder: (id: WindowId, before: WindowId | null) => void,
  active = windows[0].id,
) =>
  render(
    <WindowRail
      windows={windows}
      activeWindowId={active}
      expanded
      busyWindows={new Set()}
      onActivate={vi.fn()}
      onNewWindow={vi.fn()}
      onRename={vi.fn()}
      onClose={vi.fn()}
      onToggle={vi.fn()}
      onReorder={onReorder}
    />,
  );

const chip = (name: string) => screen.getByRole("tab", { name: new RegExp(name) });

/** jsdom gives every element a zero-sized box, so the halves need real numbers. */
const box = (element: Element, top: number) => {
  vi.spyOn(element, "getBoundingClientRect").mockReturnValue({
    top,
    bottom: top + 20,
    height: 20,
    left: 0,
    right: 40,
    width: 40,
    x: 0,
    y: top,
    toJSON: () => ({}),
  });
};

const dragFrom = (source: Element) =>
  fireEvent.dragStart(source, { dataTransfer: { setData: vi.fn(), effectAllowed: "" } });

/**
 * `clientY` is a read-only getter on MouseEvent, so fireEvent's init cannot
 * reach it — the coordinate has to be built into the event itself.
 */
const dragOverAt = (target: Element, clientY: number) => {
  const event = new MouseEvent("dragover", { bubbles: true, cancelable: true, clientY });
  Object.defineProperty(event, "dataTransfer", { value: { dropEffect: "" } });
  fireEvent(target, event);
};

describe("reorderWindow", () => {
  const [one, two, three] = [makeWindow("1"), makeWindow("2"), makeWindow("3")];

  it("moves a window in front of another", () => {
    const next = workspaceReducer(workspace(one, two, three), {
      type: "reorderWindow",
      window: three.id,
      before: one.id,
    });
    expect(names(next)).toEqual(["3", "1", "2"]);
  });

  it("moves a window to the end for a null anchor", () => {
    const next = workspaceReducer(workspace(one, two, three), {
      type: "reorderWindow",
      window: one.id,
      before: null,
    });
    expect(names(next)).toEqual(["2", "3", "1"]);
  });

  it("keeps the active window and the trees untouched", () => {
    const before = workspace(one, two, three);
    const next = workspaceReducer(before, { type: "reorderWindow", window: two.id, before: null });
    expect(next.activeWindowId).toBe(one.id);
    expect(next.windows.map((w) => w.root)).toEqual(
      expect.arrayContaining([one.root, two.root, three.root]),
    );
  });

  it("returns the same state when the drop changes nothing", () => {
    const before = workspace(one, two, three);
    // Onto itself, in front of the window it already precedes, and past the end
    // it already sits at.
    expect(workspaceReducer(before, { type: "reorderWindow", window: two.id, before: two.id })).toBe(before);
    expect(workspaceReducer(before, { type: "reorderWindow", window: one.id, before: two.id })).toBe(before);
    expect(workspaceReducer(before, { type: "reorderWindow", window: three.id, before: null })).toBe(before);
  });

  it("ignores a window or an anchor it does not know", () => {
    const before = workspace(one, two);
    const ghost = asWindowId("w-ghost");
    expect(workspaceReducer(before, { type: "reorderWindow", window: ghost, before: one.id })).toBe(before);
    expect(workspaceReducer(before, { type: "reorderWindow", window: one.id, before: ghost })).toBe(before);
  });

  it("survives a save and load as the stored order", async () => {
    const { loadWorkspace, saveWorkspace } = await import("../workspace/persistence");
    const seeded: Workspace = {
      windows: [emptyWindow("a"), emptyWindow("b")],
      activeWindowId: asWindowId("unused"),
      chrome: DEFAULT_CHROME,
    };
    const moved = workspaceReducer(
      { ...seeded, activeWindowId: seeded.windows[0].id },
      { type: "reorderWindow", window: seeded.windows[0].id, before: null },
    );
    const store = new Map<string, string>();
    const storage = {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => void store.set(key, value),
    };
    saveWorkspace(moved, storage);
    expect(names(loadWorkspace(storage))).toEqual(["b", "a"]);
  });
});

describe("window rail drag", () => {
  it("drops after the chip when the pointer is in its lower half", () => {
    const onReorder = vi.fn();
    const windows = [makeWindow("1"), makeWindow("2"), makeWindow("3")];
    renderRail(windows, onReorder);

    const target = chip("2");
    box(target, 100);
    dragFrom(chip("1"));
    dragOverAt(target, 118);
    fireEvent.drop(target.parentElement as Element);

    expect(onReorder).toHaveBeenCalledWith(windows[0].id, windows[2].id);
  });

  it("drops before the chip when the pointer is in its upper half", () => {
    const onReorder = vi.fn();
    const windows = [makeWindow("1"), makeWindow("2"), makeWindow("3")];
    renderRail(windows, onReorder);

    const target = chip("1");
    box(target, 100);
    dragFrom(chip("3"));
    dragOverAt(target, 102);
    fireEvent.drop(target.parentElement as Element);

    expect(onReorder).toHaveBeenCalledWith(windows[2].id, windows[0].id);
  });

  it("drops at the end past the last chip", () => {
    const onReorder = vi.fn();
    const windows = [makeWindow("1"), makeWindow("2")];
    renderRail(windows, onReorder);

    const target = chip("2");
    box(target, 100);
    dragFrom(chip("1"));
    dragOverAt(target, 119);
    fireEvent.drop(target.parentElement as Element);

    expect(onReorder).toHaveBeenCalledWith(windows[0].id, null);
  });

  it("marks the dragged chip and the insertion side", () => {
    const windows = [makeWindow("1"), makeWindow("2")];
    renderRail(windows, vi.fn());

    const source = chip("1");
    const target = chip("2");
    box(target, 100);
    dragFrom(source);
    dragOverAt(target, 102);

    expect(source.className).toContain("is-dragging");
    expect(target.getAttribute("data-drop")).toBe("before");

    fireEvent.dragEnd(source);
    expect(source.className).not.toContain("is-dragging");
    expect(target.getAttribute("data-drop")).toBeNull();
  });

  it("reorders nothing on a drag that never crossed a chip", () => {
    const onReorder = vi.fn();
    const windows = [makeWindow("1"), makeWindow("2")];
    renderRail(windows, onReorder);

    const source = chip("1");
    dragFrom(source);
    fireEvent.drop(source.parentElement as Element);

    expect(onReorder).not.toHaveBeenCalled();
  });

  it("moves the active window with alt and the arrows", () => {
    const onReorder = vi.fn();
    const windows = [makeWindow("1"), makeWindow("2"), makeWindow("3")];
    renderRail(windows, onReorder, windows[1].id);

    fireEvent.keyDown(chip("2"), { key: "ArrowDown", altKey: true });
    expect(onReorder).toHaveBeenLastCalledWith(windows[1].id, null);

    fireEvent.keyDown(chip("2"), { key: "ArrowUp", altKey: true });
    expect(onReorder).toHaveBeenLastCalledWith(windows[1].id, windows[0].id);
  });

  it("holds at the ends rather than wrapping", () => {
    const onReorder = vi.fn();
    const windows = [makeWindow("1"), makeWindow("2")];
    renderRail(windows, onReorder, windows[0].id);

    fireEvent.keyDown(chip("1"), { key: "ArrowUp", altKey: true });
    expect(onReorder).not.toHaveBeenCalled();
  });
});
