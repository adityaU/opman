/**
 * The terminal busy poll. The behaviours that matter are that it costs nothing
 * when there are no terminals, that it covers panes in windows that are not
 * mounted, and that it does not wake its consumers when nothing changed.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

const ptySessions = vi.fn();
vi.mock("../api", () => ({ ptySessions: (...args: unknown[]) => ptySessions(...args) }));

const { useTerminalActivity } = await import("../workspace/useTerminalActivity");
import { asPaneId, asWindowId, type WorkspaceWindow } from "../workspace/types";

const terminalWindow = (name: string, paneId: string, ptyId: string | null): WorkspaceWindow => ({
  id: asWindowId(`w-${name}`),
  name,
  focusedPaneId: asPaneId(paneId),
  zoomedPaneId: null,
  root: {
    type: "leaf",
    id: asPaneId(paneId),
    widget: { kind: "terminal", projectPath: "/repo", ptyId },
  },
});

const chatWindow = (paneId: string): WorkspaceWindow => ({
  id: asWindowId("w-chat"),
  name: "chat",
  focusedPaneId: asPaneId(paneId),
  zoomedPaneId: null,
  root: {
    type: "leaf",
    id: asPaneId(paneId),
    widget: { kind: "chat", projectPath: "/repo", sessionId: "s1", engine: null },
  },
});

/** What the sessions endpoint returns, with only the fields the hook reads. */
const running = (...ids: string[]) =>
  ids.map((id) => ({
    id,
    kind: "shell" as const,
    label: id,
    project: "/repo",
    activity: "running" as const,
  }));

const idle = (...ids: string[]) =>
  ids.map((id) => ({
    id,
    kind: "shell" as const,
    label: id,
    project: "/repo",
    activity: "idle" as const,
  }));

/** Advance fake timers and let the resulting state land. */
async function tick(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

beforeEach(() => {
  ptySessions.mockReset();
  ptySessions.mockResolvedValue([]);
});
afterEach(() => vi.useRealTimers());

describe("useTerminalActivity", () => {
  it("makes no request when there is no terminal pane", async () => {
    const { result } = renderHook(() => useTerminalActivity([chatWindow("p1")]));
    await waitFor(() => expect(result.current.size).toBe(0));
    expect(ptySessions).not.toHaveBeenCalled();
  });

  it("makes no request for a terminal pane that has no shell yet", async () => {
    renderHook(() => useTerminalActivity([terminalWindow("1", "p1", null)]));
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(ptySessions).not.toHaveBeenCalled();
  });

  it("marks a pane busy when its shell is running a command", async () => {
    ptySessions.mockResolvedValue([...idle("a"), ...running("b")]);
    const { result } = renderHook(() => useTerminalActivity([terminalWindow("1", "p1", "b")]));
    await waitFor(() => expect(result.current.has(asPaneId("p1"))).toBe(true));
  });

  it("leaves a pane idle when the busy shell is not its own", async () => {
    ptySessions.mockResolvedValue([...idle("a"), ...running("other")]);
    const { result } = renderHook(() => useTerminalActivity([terminalWindow("1", "p1", "a")]));
    await waitFor(() => expect(ptySessions).toHaveBeenCalled());
    expect(result.current.has(asPaneId("p1"))).toBe(false);
  });

  it("leaves a pane idle when its shell has gone from the listing", async () => {
    // An exited shell is absent rather than idle, and must not read as busy.
    ptySessions.mockResolvedValue(running("someone-else"));
    const { result } = renderHook(() => useTerminalActivity([terminalWindow("1", "p1", "gone")]));
    await waitFor(() => expect(ptySessions).toHaveBeenCalled());
    expect(result.current.has(asPaneId("p1"))).toBe(false);
  });

  it("covers a pane in a window that is not the active one", async () => {
    // Background windows are not mounted, so their terminals have no output
    // stream — the poll is the only thing that can see them.
    ptySessions.mockResolvedValue(running("bg"));
    const { result } = renderHook(() =>
      useTerminalActivity([terminalWindow("1", "p1", "fg"), terminalWindow("2", "p2", "bg")]),
    );
    await waitFor(() => expect(result.current.has(asPaneId("p2"))).toBe(true));
    expect(result.current.has(asPaneId("p1"))).toBe(false);
  });

  it("keeps the same set object when nothing changed", async () => {
    vi.useFakeTimers();
    ptySessions.mockResolvedValue(running("a"));
    const { result } = renderHook(() => useTerminalActivity([terminalWindow("1", "p1", "a")]));

    await tick(10);
    const first = result.current;
    expect(first.has(asPaneId("p1"))).toBe(true);

    await tick(2100);
    expect(ptySessions).toHaveBeenCalledTimes(2);
    // Identity is the contract: a new Set every poll would re-render every pane
    // twice a second for as long as a terminal is open.
    expect(result.current).toBe(first);
  });

  it("survives a failing request rather than stopping the poll", async () => {
    vi.useFakeTimers();
    ptySessions.mockRejectedValueOnce(new Error("offline"));
    ptySessions.mockResolvedValue(running("a"));
    const { result } = renderHook(() => useTerminalActivity([terminalWindow("1", "p1", "a")]));

    await tick(10);
    expect(result.current.size).toBe(0);
    await tick(2100);
    expect(result.current.has(asPaneId("p1"))).toBe(true);
  });
});
