import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import type { ProjectInfo, SessionInfo } from "../api";

// A plain function rather than `vi.fn`: vitest tracks a mock's result, and a
// rejected one is reported as a test error even where the code under test catches
// it — which is exactly the path this suite needs to assert.
type Page = { sessions: SessionInfo[]; session_count: number };
let respond: (project: number, arg: unknown) => Promise<Page> = async () => ({
  sessions: [],
  session_count: 0,
});
const calls: unknown[][] = [];

vi.mock("../api", async () => ({
  ...(await vi.importActual<typeof import("../api")>("../api")),
  fetchSessionPage: (project: number, arg: unknown) => {
    calls.push([project, arg]);
    return respond(project, arg);
  },
}));

const { useSessionPages } = await import("../sidebar/useSessionPages");

function session(id: string, updated = 1): SessionInfo {
  return {
    id,
    title: id,
    parentID: "",
    directory: "/p",
    time: { created: updated, updated },
  };
}

function project(sessions: SessionInfo[], total: number): ProjectInfo {
  return {
    name: "p",
    path: "/p",
    index: 0,
    active_session: null,
    sessions,
    session_count: total,
    git_branch: "",
    busy_sessions: [],
  };
}

beforeEach(() => {
  calls.length = 0;
  respond = async () => ({ sessions: [], session_count: 0 });
});

const NONE = new Set<string>();

describe("useSessionPages", () => {
  it("reports what the server still holds back", () => {
    const { result } = renderHook(() =>
      useSessionPages([project([session("a"), session("b")], 25)], 0, NONE),
    );

    expect(result.current.remaining).toBe(23);
  });

  it("appends a fetched page to the list it already had", async () => {
    respond = async () => ({ sessions: [session("b")], session_count: 2 });
    const { result } = renderHook(() =>
      useSessionPages([project([session("a")], 2)], 0, NONE),
    );

    await act(() => result.current.loadMore());

    expect(calls).toEqual([[0, { offset: 1, limit: 20 }]]);
    expect(result.current.project?.sessions.map((s) => s.id).sort()).toEqual(["a", "b"]);
    expect(result.current.remaining).toBe(0);
  });

  it("does not ask for a page it already has", async () => {
    const { result } = renderHook(() =>
      useSessionPages([project([session("a")], 1)], 0, NONE),
    );

    await act(() => result.current.loadMore());

    expect(calls).toEqual([]);
  });

  it("keeps the list usable when a page fetch fails", async () => {
    respond = () => Promise.reject(new Error("offline"));
    const { result } = renderHook(() =>
      useSessionPages([project([session("a")], 5)], 0, NONE),
    );

    await act(() => result.current.loadMore());

    expect(result.current.loading).toBe(false);
    expect(result.current.project?.sessions.map((s) => s.id)).toEqual(["a"]);
    expect(result.current.remaining).toBe(4);
  });

  it("re-fetches a pinned session that is older than the first page", async () => {
    respond = async () => ({ sessions: [session("pinned")], session_count: 40 });
    const { result } = renderHook(() =>
      useSessionPages([project([session("a")], 40)], 0, new Set(["pinned"])),
    );

    await waitFor(() =>
      expect(result.current.project?.sessions.map((s) => s.id).sort()).toEqual([
        "a",
        "pinned",
      ]),
    );
    expect(calls).toEqual([[0, { ids: ["pinned"] }]]);
  });

  it("does not re-fetch a pinned session the state snapshot already carries", async () => {
    const { result } = renderHook(() =>
      useSessionPages([project([session("a")], 1)], 0, new Set(["a"])),
    );

    await waitFor(() => expect(result.current.remaining).toBe(0));
    expect(calls).toEqual([]);
  });

  it("lets a fresh snapshot win over a stale extra page", async () => {
    respond = async () => ({
      sessions: [{ ...session("b"), title: "old title" }],
      session_count: 2,
    });
    const { result, rerender } = renderHook(
      ({ projects }) => useSessionPages(projects, 0, NONE),
      { initialProps: { projects: [project([session("a")], 2)] } },
    );

    await act(() => result.current.loadMore());
    rerender({
      projects: [project([session("a"), { ...session("b"), title: "renamed" }], 2)],
    });

    const b = result.current.project?.sessions.find((s) => s.id === "b");
    expect(b?.title).toBe("renamed");
  });
});
