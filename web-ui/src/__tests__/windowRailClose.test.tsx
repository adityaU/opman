/**
 * Closing a window from the rail. What matters is that the × asks before it
 * acts, that Keep leaves the workspace alone, and that the last window has no
 * × at all — the reducer refuses to close it, so offering the control would be
 * asking a question with no answer.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { WindowRail } from "../workspace/WindowRail";
import { asPaneId, asWindowId, type WindowId, type WorkspaceWindow } from "../workspace/types";
import { EMPTY_HISTORY } from "../workspace/history";

const makeWindow = (name: string): WorkspaceWindow => ({
  id: asWindowId(`w-${name}`),
  name,
  focusedPaneId: asPaneId(`p-${name}`),
  zoomedPaneId: null,
  root: { type: "leaf", id: asPaneId(`p-${name}`), widget: null, history: EMPTY_HISTORY },
});

const renderRail = (windows: readonly WorkspaceWindow[], onClose: (id: WindowId) => void) =>
  render(
    <WindowRail
      windows={windows}
      activeWindowId={windows[0].id}
      expanded
      busyWindows={new Set()}
      onActivate={vi.fn()}
      onNewWindow={vi.fn()}
      onRename={vi.fn()}
      onClose={onClose}
      onToggle={vi.fn()}
      onReorder={vi.fn()}
    />,
  );

describe("window rail close", () => {
  it("asks before closing, and closes on confirm", () => {
    const onClose = vi.fn();
    renderRail([makeWindow("1"), makeWindow("2")], onClose);

    fireEvent.click(screen.getByLabelText("Close 2"));
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledWith(asWindowId("w-2"));
  });

  it("leaves the window alone when the answer is Keep", () => {
    const onClose = vi.fn();
    renderRail([makeWindow("1"), makeWindow("2")], onClose);

    fireEvent.click(screen.getByLabelText("Close 2"));
    fireEvent.click(screen.getByRole("button", { name: "Keep" }));

    expect(onClose).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog", { name: "Close window" })).toBeNull();
  });

  it("offers no × on the last window", () => {
    renderRail([makeWindow("1")], vi.fn());
    expect(screen.queryByLabelText("Close 1")).toBeNull();
  });

  it("does not activate the window behind the ×", () => {
    const onActivate = vi.fn();
    const windows = [makeWindow("1"), makeWindow("2")];
    render(
      <WindowRail
        windows={windows}
        activeWindowId={windows[0].id}
        expanded
        busyWindows={new Set()}
        onActivate={onActivate}
        onNewWindow={vi.fn()}
        onRename={vi.fn()}
        onClose={vi.fn()}
        onToggle={vi.fn()}
      onReorder={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByLabelText("Close 2"));
    expect(onActivate).not.toHaveBeenCalled();
  });
});
