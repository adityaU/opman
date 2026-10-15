/**
 * A pane that switches runner must follow its conversation into the handoff session.
 * The send used to report the old session id, so the pane stayed on the conversation it
 * had just left and its next send went back to the old runner.
 */
import { describe, expect, it, vi } from "vitest";
import { renderHook } from "@testing-library/react";
import { readHandoff } from "../api";
import { useWorkspaceShellProps, type WorkspaceShellDeps } from "../workspace/useWorkspaceShellProps";

const sendMessage = vi.fn();
vi.mock("../api", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("../api");
  return { ...actual, sendMessage: (...args: unknown[]) => sendMessage(...args) };
});

const ENGINE = { runner: "codex", model: null, agent: "", effort: null, permission: "default" };

function deps(overrides: Partial<WorkspaceShellDeps> = {}): WorkspaceShellDeps {
  return {
    appState: { projects: [{ path: "/p", sessions: [] }] } as never,
    activeSessionId: null,
    busySessions: new Set(),
    defaultEngine: ENGINE,
    availableRunners: ["claude", "codex"],
    openModelPicker: vi.fn(),
    openAgentPicker: vi.fn(),
    runSlashCommand: vi.fn(),
    onError: vi.fn(),
    subagentMessages: new Map(),
    isBookmarked: () => false,
    toggleBookmark: vi.fn(),
    openSession: vi.fn(),
    permissions: [],
    questions: [],
    onPermissionReply: vi.fn(),
    onQuestionReply: vi.fn(),
    onQuestionDismiss: vi.fn(),
    searchOpen: false,
    closeSearch: vi.fn(),
    ...overrides,
  };
}

describe("readHandoff", () => {
  it("reads a switch reply and ignores everything else", () => {
    expect(readHandoff({ switched: true, session_id: "new", runner: "codex" }))
      .toEqual({ sessionId: "new", runner: "codex" });
    expect(readHandoff({ switched: false, session_id: "old" })).toBeNull();
    expect(readHandoff({ switched: true })).toBeNull();
    expect(readHandoff(null)).toBeNull();
    expect(readHandoff("ok")).toBeNull();
  });
});

describe("pane send", () => {
  const target = { sessionId: "old", projectPath: "/p", engine: ENGINE, switchRunner: true };

  it("returns the handoff session so the pane binds to it", async () => {
    sendMessage.mockResolvedValueOnce({ switched: true, session_id: "new", runner: "codex" });
    const onHandoff = vi.fn();
    const { result } = renderHook(() => useWorkspaceShellProps(deps({ onHandoff })));
    const reply = await result.current.workspaceProps.chat.send(target, "hi");
    expect(reply).toEqual({ ok: true, sessionId: "new" });
    expect(onHandoff).toHaveBeenCalledWith("codex");
  });

  it("keeps the session when the send did not switch", async () => {
    sendMessage.mockResolvedValueOnce({});
    const { result } = renderHook(() => useWorkspaceShellProps(deps()));
    expect(await result.current.workspaceProps.chat.send({ ...target, switchRunner: false }, "hi"))
      .toEqual({ ok: true, sessionId: "old" });
  });
});
