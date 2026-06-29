/**
 * Unit tests for useKanbanViewState — the Kanban board's own path-based route
 * (`/kanban`). Verifies navigation writes the right URLs and that view state
 * recomputes from the path on a programmatic location change (e.g. when
 * selecting a session navigates back to "/").
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useKanbanViewState } from "../kanban/useKanbanViewState";
import { LOCATION_CHANGE_EVENT } from "../utils/navigation";

const pushStateSpy = vi.fn();
const replaceStateSpy = vi.fn();

Object.defineProperty(window, "location", {
  value: { search: "", pathname: "/", reload: vi.fn() },
  writable: true,
});
Object.defineProperty(window, "history", {
  value: { state: null, pushState: pushStateSpy, replaceState: replaceStateSpy },
  writable: true,
});

beforeEach(() => {
  window.location.search = "";
  window.location.pathname = "/";
  pushStateSpy.mockClear();
  replaceStateSpy.mockClear();
});
afterEach(() => vi.restoreAllMocks());

describe("useKanbanViewState", () => {
  it("is not in kanban view on the chat path", () => {
    window.location.pathname = "/";
    const { result } = renderHook(() => useKanbanViewState());
    expect(result.current.isKanbanView).toBe(false);
  });

  it("detects the kanban view from the path and reads the focus task", () => {
    window.location.pathname = "/kanban";
    window.location.search = "?project=2&task=t9";
    const { result } = renderHook(() => useKanbanViewState());
    expect(result.current.isKanbanView).toBe(true);
    expect(result.current.focusTaskId).toBe("t9");
  });

  it("openKanban navigates to /kanban with the project", () => {
    const { result } = renderHook(() => useKanbanViewState());
    act(() => result.current.openKanban(3));
    expect(pushStateSpy).toHaveBeenCalledWith(null, "", "/kanban?project=3");
  });

  it("openKanbanTask navigates to /kanban with project + task", () => {
    const { result } = renderHook(() => useKanbanViewState());
    act(() => result.current.openKanbanTask("task42", 1));
    expect(pushStateSpy).toHaveBeenCalledWith(null, "", "/kanban?project=1&task=task42");
  });

  it("clearFocusTask drops only the task param via replaceState", () => {
    window.location.pathname = "/kanban";
    window.location.search = "?project=2&task=t9";
    const { result } = renderHook(() => useKanbanViewState());
    act(() => result.current.clearFocusTask());
    expect(replaceStateSpy).toHaveBeenCalledWith(null, "", "/kanban?project=2");
  });

  it("clearFocusTask is a no-op when not on the board", () => {
    window.location.pathname = "/";
    const { result } = renderHook(() => useKanbanViewState());
    act(() => result.current.clearFocusTask());
    expect(replaceStateSpy).not.toHaveBeenCalled();
  });

  it("leaves kanban view when a location change navigates back to chat", () => {
    window.location.pathname = "/kanban";
    const { result } = renderHook(() => useKanbanViewState());
    expect(result.current.isKanbanView).toBe(true);

    // Simulate setUrlSession navigating to "/" and emitting a location change.
    act(() => {
      window.location.pathname = "/";
      window.location.search = "?session=s1&project=0";
      window.dispatchEvent(new Event(LOCATION_CHANGE_EVENT));
    });
    expect(result.current.isKanbanView).toBe(false);
  });
});
