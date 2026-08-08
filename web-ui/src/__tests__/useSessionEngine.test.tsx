/**
 * The composer belongs to the session, not to the runner.
 *
 * Every case here is a shape of the reported bug: open session B and find session A's
 * model in the composer, or find an effort and a permission that were never chosen for
 * either. The runner is the source of truth for all four values, so these tests assert
 * that what the session row reports is what the composer shows.
 */
import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useSessionEngine } from "../hooks/useSessionEngine";
import { useRunnerConfig } from "../hooks/useRunnerConfig";

const setSessionEngine = vi.fn().mockResolvedValue(true);
vi.mock("../api", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("../api");
  return { ...actual, setSessionEngine: (...args: unknown[]) => setSessionEngine(...args) };
});

const providers = {
  all: [
    { id: "anthropic", models: { "claude-opus-4-5": {}, "claude-sonnet-4-5": {} } },
    { id: "openai", models: { "gpt-5": {} } },
  ],
};

interface SessionRow {
  id: string;
  runner?: string;
  engine?: { model?: string; agent?: string; effort?: string; permissionMode?: string };
}

/** Drive the hook the way ChatLayout does, with the model/agent store alongside it. */
function mountFor(session: SessionRow | undefined, runner = "claude") {
  const store: { model: { providerID: string; modelID: string } | null; agent: string } = {
    model: null,
    agent: "",
  };
  const view = renderHook(
    ({ activeSession, currentRunner }: { activeSession?: SessionRow; currentRunner: string }) => {
      const runnerConfig = useRunnerConfig();
      return useSessionEngine({
        activeSessionId: activeSession?.id ?? null,
        activeSession: activeSession as never,
        currentRunner,
        runnerConfig,
        providers,
        transcriptModel: null,
        selectedModel: store.model,
        setSelectedModel: (next) => { store.model = next; },
        selectedAgent: store.agent,
        setSelectedAgent: (next) => { store.agent = next; },
      });
    },
    { initialProps: { activeSession: session, currentRunner: runner } },
  );
  return { view, store };
}

describe("useSessionEngine", () => {
  beforeEach(() => {
    localStorage.clear();
    setSessionEngine.mockClear();
  });

  it("shows what the session's runner reports", async () => {
    const { view, store } = mountFor({
      id: "ses_a",
      runner: "claude",
      engine: { model: "claude-opus-4-5", agent: "plan", effort: "high", permissionMode: "acceptEdits" },
    });

    await waitFor(() => expect(store.model).toEqual({ providerID: "anthropic", modelID: "claude-opus-4-5" }));
    expect(store.agent).toBe("plan");
    expect(view.result.current.effort).toBe("high");
    expect(view.result.current.permission).toBe("acceptEdits");
  });

  it("swaps every control when the session changes", async () => {
    const a: SessionRow = {
      id: "ses_a",
      runner: "claude",
      engine: { model: "claude-opus-4-5", agent: "plan", effort: "high", permissionMode: "acceptEdits" },
    };
    const b: SessionRow = {
      id: "ses_b",
      runner: "claude",
      engine: { model: "claude-sonnet-4-5", agent: "build", effort: "low", permissionMode: "default" },
    };
    const { view, store } = mountFor(a);
    await waitFor(() => expect(store.agent).toBe("plan"));

    act(() => view.rerender({ activeSession: b, currentRunner: "claude" }));

    // The whole point: none of session A's values survive into session B.
    await waitFor(() => expect(store.model).toEqual({ providerID: "anthropic", modelID: "claude-sonnet-4-5" }));
    expect(store.agent).toBe("build");
    expect(view.result.current.effort).toBe("low");
    expect(view.result.current.permission).toBe("default");
  });

  it("records a change with the session that owns it", async () => {
    const { view } = mountFor({ id: "ses_a", runner: "claude", engine: { model: "claude-opus-4-5" } });
    await waitFor(() => expect(view.result.current.permission).toBeDefined());

    act(() => view.result.current.setEffort("medium"));

    expect(setSessionEngine).toHaveBeenCalledWith("ses_a", { effort: "medium" });
    // Only the field that changed: restating the others would let a stale value in the
    // composer overwrite a good one on the runner.
    const [, patch] = setSessionEngine.mock.calls.at(-1)!;
    expect(Object.keys(patch as object)).toEqual(["effort"]);
  });

  it("does not tell a runner about a session that does not exist yet", async () => {
    const { view } = mountFor(undefined);
    act(() => view.result.current.setPermission("never"));

    expect(setSessionEngine).not.toHaveBeenCalled();
    // It is still remembered for the runner, so the new session opens on it.
    expect(view.result.current.permission).toBe("never");
  });

  it("ignores a configuration belonging to the runner being switched away from", async () => {
    // The row still names the old runner while the switch is pending; its model is from
    // a catalogue the new runner has never heard of.
    const { view, store } = mountFor(
      { id: "ses_a", runner: "claude", engine: { model: "claude-opus-4-5", agent: "plan" } },
      "codex",
    );
    await waitFor(() => expect(view.result.current.permission).toBeDefined());
    expect(store.model).not.toEqual({ providerID: "anthropic", modelID: "claude-opus-4-5" });
    expect(store.agent).toBe("");
  });

  it("falls back to the runner's remembered setup for a session never configured", async () => {
    const { view, store } = mountFor({ id: "ses_new", runner: "claude", engine: {} });
    await waitFor(() => expect(view.result.current.permission).toBe("default"));
    expect(store.agent).toBe("");
    expect(view.result.current.effort).toBeNull();
  });
});
