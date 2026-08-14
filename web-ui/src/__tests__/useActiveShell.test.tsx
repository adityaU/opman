/**
 * Which shell a terminal shows.
 *
 * The behaviours that matter are that a shell is never ended by the view
 * changing, that a remembered shell which has since exited sends the user to
 * the picker rather than to a fresh prompt pretending to be it, and that a
 * restored pane is not sent to the picker merely because the list has not
 * arrived yet.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

const ptySessions = vi.fn();
const spawnPty = vi.fn();
const ptyKill = vi.fn();
const ptyRename = vi.fn();

vi.mock("../api", () => ({
  ptySessions: (...args: unknown[]) => ptySessions(...args),
  spawnPty: (...args: unknown[]) => spawnPty(...args),
  ptyKill: (...args: unknown[]) => ptyKill(...args),
  ptyRename: (...args: unknown[]) => ptyRename(...args),
}));

let nextId = 0;
vi.mock("../utils/uuid", () => ({ uuid: () => `new-${++nextId}` }));

const { useActiveShell } = await import("../terminal-panel/useActiveShell");
const { invalidateShells } = await import("../terminal-panel/useShells");

const shell = (id: string, activity: "idle" | "running" = "idle", project = "/repo") => ({
  id,
  kind: "shell" as const,
  label: id,
  project,
  activity,
});

beforeEach(() => {
  nextId = 0;
  invalidateShells();
  ptySessions.mockReset();
  spawnPty.mockReset().mockResolvedValue({ id: "x", ok: true });
  ptyKill.mockReset().mockResolvedValue(undefined);
  ptyRename.mockReset().mockResolvedValue(undefined);
  ptySessions.mockResolvedValue([]);
});

const mount = (ptyId: string | null, onChange = vi.fn()) =>
  renderHook(() => useActiveShell(ptyId, "/repo", null, onChange));

describe("useActiveShell", () => {
  it("shows the picker when no shell has been chosen", async () => {
    const { result } = mount(null);
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.choosing).toBe(true);
  });

  it("shows a shell it was handed without asking anything", async () => {
    ptySessions.mockResolvedValue([shell("a")]);
    const { result } = mount("a");
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.choosing).toBe(false);
    expect(result.current.shell?.id).toBe("a");
  });

  it("only offers shells belonging to its own project", async () => {
    ptySessions.mockResolvedValue([shell("mine"), shell("theirs", "idle", "/other")]);
    const { result } = mount(null);
    await waitFor(() => expect(result.current.shells.length).toBe(1));
    expect(result.current.shells[0].id).toBe("mine");
  });

  /** The point of the whole change: a shell is not the view's to end. */
  it("never kills anything just because it was mounted and unmounted", async () => {
    ptySessions.mockResolvedValue([shell("a")]);
    const { result, unmount } = mount("a");
    await waitFor(() => expect(result.current.shell?.id).toBe("a"));
    unmount();
    expect(ptyKill).not.toHaveBeenCalled();
  });

  it("falls back to the picker when its shell has exited", async () => {
    ptySessions.mockResolvedValue([shell("someone-else")]);
    const onChange = vi.fn();
    const { result } = mount("gone", onChange);
    await waitFor(() => expect(result.current.choosing).toBe(true));
    // Reported, so the owner stops trying to restore a shell that is not there.
    expect(onChange).toHaveBeenCalledWith(null);
  });

  it("does not abandon its shell before the list has arrived", async () => {
    let release: (value: unknown[]) => void = () => {};
    ptySessions.mockReturnValue(new Promise((resolve) => (release = resolve)));
    const onChange = vi.fn();
    const { result } = mount("a", onChange);
    // Still loading: every id looks dead, and dropping it here would send a
    // restored pane to the picker on every single reload.
    expect(result.current.choosing).toBe(false);
    expect(onChange).not.toHaveBeenCalled();
    await act(async () => release([shell("a")]));
    expect(result.current.choosing).toBe(false);
  });

  it("reports the shell it started so the owner can persist it", async () => {
    ptySessions.mockResolvedValue([]);
    const onChange = vi.fn();
    const { result } = mount(null, onChange);
    await waitFor(() => expect(result.current.loading).toBe(false));

    ptySessions.mockResolvedValue([shell("new-1")]);
    await act(async () => result.current.create("shell"));

    expect(spawnPty).toHaveBeenCalledWith("shell", "new-1", 24, 80, {
      project: "/repo",
      sessionId: undefined,
    });
    expect(onChange).toHaveBeenCalledWith("new-1");
  });

  it("killing another pane's shell leaves this one showing its own", async () => {
    ptySessions.mockResolvedValue([shell("mine"), shell("other")]);
    const onChange = vi.fn();
    const { result } = mount("mine", onChange);
    await waitFor(() => expect(result.current.shell?.id).toBe("mine"));

    ptySessions.mockResolvedValue([shell("mine")]);
    await act(async () => result.current.kill("other"));

    expect(ptyKill).toHaveBeenCalledWith("other");
    expect(onChange).not.toHaveBeenCalledWith(null);
    expect(result.current.choosing).toBe(false);
  });

  it("killing its own shell asks which one next", async () => {
    ptySessions.mockResolvedValue([shell("mine")]);
    const onChange = vi.fn();
    const { result } = mount("mine", onChange);
    await waitFor(() => expect(result.current.shell?.id).toBe("mine"));

    ptySessions.mockResolvedValue([]);
    await act(async () => result.current.kill("mine"));
    expect(onChange).toHaveBeenCalledWith(null);
  });

  it("steps through the project's shells and wraps", async () => {
    ptySessions.mockResolvedValue([shell("a"), shell("b")]);
    const onChange = vi.fn();
    const { result } = mount("a", onChange);
    await waitFor(() => expect(result.current.shells.length).toBe(2));

    act(() => result.current.step(1));
    expect(onChange).toHaveBeenLastCalledWith("b");
    act(() => result.current.step(1));
    expect(onChange).toHaveBeenLastCalledWith("a");
    act(() => result.current.step(-1));
    expect(onChange).toHaveBeenLastCalledWith("b");
  });

  it("renames a shell by id, trimmed, and refuses a blank name", async () => {
    ptySessions.mockResolvedValue([shell("a")]);
    const { result } = mount("a");
    await waitFor(() => expect(result.current.shell?.id).toBe("a"));

    await act(async () => result.current.renameById("a", "  Build  "));
    expect(ptyRename).toHaveBeenCalledWith("a", "Build");

    ptyRename.mockClear();
    await act(async () => result.current.renameById("a", "   "));
    expect(ptyRename).not.toHaveBeenCalled();
  });

  it("can be asked to switch and can back out of switching", async () => {
    ptySessions.mockResolvedValue([shell("a"), shell("b")]);
    const { result } = mount("a");
    await waitFor(() => expect(result.current.shell?.id).toBe("a"));

    act(() => result.current.startChoosing());
    expect(result.current.choosing).toBe(true);
    act(() => result.current.stopChoosing());
    expect(result.current.choosing).toBe(false);
  });
});
