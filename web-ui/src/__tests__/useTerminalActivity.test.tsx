/**
 * The terminal busy poll. The behaviours that matter are that it costs nothing
 * when there are no terminals, that it covers panes in windows that are not
 * mounted, and that it does not wake its consumers when nothing changed.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

const ptyActivity = vi.fn();
vi.mock("../api", () => ({ ptyActivity: (...args: unknown[]) => ptyActivity(...args) }));

const { useTerminalActivity } = await import("../workspace/useTerminalActivity");
import { asPaneId, asWindowId, type WorkspaceWindow } from "../workspace/types";

const terminalWindow = (name: string, paneId: string, ptyIds: string[]): WorkspaceWindow => ({
  id: asWindowId(`w-${name}`),
  name,
  focusedPaneId: asPaneId(paneId),
  zoomedPaneId: null,
  root: {
    type: "leaf",
    id: asPaneId(paneId),
    widget: { kind: "terminal", projectPath: "/repo", ptyIds },
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

/** Advance fake timers and let the resulting state land. */
async function tick(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

beforeEach(() => {
  ptyActivity.mockReset();
  ptyActivity.mockResolvedValue({});
});
afterEach(() => vi.useRealTimers());

describe("useTerminalActivity", () => {
  it("makes no request when there is no terminal pane", async () => {
    const { result } = renderHook(() => useTerminalActivity([chatWindow("p1")]));
    await waitFor(() => expect(result.current.size).toBe(0));
    expect(ptyActivity).not.toHaveBeenCalled();
  });

  it("makes no request for a terminal pane that owns no PTY yet", async () => {
    renderHook(() => useTerminalActivity([terminalWindow("1", "p1", [])]));
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(ptyActivity).not.toHaveBeenCalled();
  });

  it("marks a pane busy when any of its PTYs is running a command", async () => {
    ptyActivity.mockResolvedValue({ a: "idle", b: "running" });
    const { result } = renderHook(() =>
      useTerminalActivity([terminalWindow("1", "p1", ["a", "b"])]),
    );
    await waitFor(() => expect(result.current.has(asPaneId("p1"))).toBe(true));
  });

  it("leaves a pane idle when every PTY of its own is idle", async () => {
    ptyActivity.mockResolvedValue({ a: "idle", other: "running" });
    const { result } = renderHook(() => useTerminalActivity([terminalWindow("1", "p1", ["a"])]));
    await waitFor(() => expect(ptyActivity).toHaveBeenCalled());
    expect(result.current.has(asPaneId("p1"))).toBe(false);
  });

  it("covers a pane in a window that is not the active one", async () => {
    // Background windows are not mounted, so their terminals have no output
    // stream — the poll is the only thing that can see them.
    ptyActivity.mockResolvedValue({ bg: "running" });
    const { result } = renderHook(() =>
      useTerminalActivity([terminalWindow("1", "p1", ["fg"]), terminalWindow("2", "p2", ["bg"])]),
    );
    await waitFor(() => expect(result.current.has(asPaneId("p2"))).toBe(true));
    expect(result.current.has(asPaneId("p1"))).toBe(false);
  });

  it("keeps the same set object when nothing changed", async () => {
    vi.useFakeTimers();
    ptyActivity.mockResolvedValue({ a: "running" });
    const { result } = renderHook(() => useTerminalActivity([terminalWindow("1", "p1", ["a"])]));

    await tick(10);
    const first = result.current;
    expect(first.has(asPaneId("p1"))).toBe(true);

    await tick(2100);
    expect(ptyActivity).toHaveBeenCalledTimes(2);
    // Identity is the contract: a new Set every poll would re-render every pane
    // twice a second for as long as a terminal is open.
    expect(result.current).toBe(first);
  });

  it("survives a failing request rather than stopping the poll", async () => {
    vi.useFakeTimers();
    ptyActivity.mockRejectedValueOnce(new Error("offline"));
    ptyActivity.mockResolvedValue({ a: "running" });
    const { result } = renderHook(() => useTerminalActivity([terminalWindow("1", "p1", ["a"])]));

    await tick(10);
    expect(result.current.size).toBe(0);
    await tick(2100);
    expect(result.current.has(asPaneId("p1"))).toBe(true);
  });
});
