/**
 * Switching runner, then model, then re-sending: each step must wait until the composer
 * shows the previous one, or the send goes out on the old engine.
 */
import { describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { CONTINUE_PROMPT, MODEL_WAIT_MS, useLimitSwitch, type LimitSwitchHost } from "../runner-limit/useLimitSwitch";

const GPT = { providerID: "openai", modelID: "gpt-5.6-sol" };

function host(overrides: Partial<LimitSwitchHost> = {}): LimitSwitchHost {
  return {
    currentRunner: "claude",
    selectedModel: { providerID: "anthropic", modelID: "sonnet" },
    onRunnerChange: vi.fn(),
    onModelSelected: vi.fn(),
    onSend: vi.fn().mockResolvedValue(true),
    ...overrides,
  };
}

describe("useLimitSwitch", () => {
  it("changes runner, then model, then re-sends — each after the last took effect", async () => {
    const start = host();
    const { result, rerender } = renderHook((props: LimitSwitchHost) => useLimitSwitch(props), {
      initialProps: start,
    });

    act(() => result.current.switchTo({ runner: "codex", model: GPT }, "fix the bug"));
    expect(start.onRunnerChange).toHaveBeenCalledWith("codex");
    expect(start.onModelSelected).not.toHaveBeenCalled();
    expect(start.onSend).not.toHaveBeenCalled();

    // The runner landed, with the model cleared.
    rerender({ ...start, currentRunner: "codex", selectedModel: null });
    expect(start.onModelSelected).toHaveBeenCalledWith("gpt-5.6-sol", "openai");
    expect(start.onSend).not.toHaveBeenCalled();

    // The model landed: now, and only now, the prompt goes out.
    await act(async () => rerender({ ...start, currentRunner: "codex", selectedModel: GPT }));
    expect(start.onSend).toHaveBeenCalledWith("fix the bug");
    expect(result.current.switching).toBe(false);
  });

  it("hands off and continues immediately when there is no failed prompt", async () => {
    const start = host();
    const { result, rerender } = renderHook((props: LimitSwitchHost) => useLimitSwitch(props), {
      initialProps: start,
    });
    act(() => result.current.switchTo({ runner: "codex", model: null }));
    await act(async () => rerender({ ...start, currentRunner: "codex", selectedModel: null }));
    expect(start.onModelSelected).not.toHaveBeenCalled();
    expect(start.onSend).toHaveBeenCalledWith(CONTINUE_PROMPT);
    expect(result.current.switching).toBe(false);
  });

  it("only selects the target in a new pane with no previous session", () => {
    const start = host({ hasSession: false });
    const { result, rerender } = renderHook((props: LimitSwitchHost) => useLimitSwitch(props), { initialProps: start });
    act(() => result.current.switchTo({ runner: "codex", model: null }));
    rerender({ ...start, currentRunner: "codex", selectedModel: null });
    expect(start.onSend).not.toHaveBeenCalled();
    expect(result.current.switching).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("waits for the selected permission after the runner and model before retrying", async () => {
    const start = host({ selectedPermission: "default", onPermissionChange: vi.fn() });
    const { result, rerender } = renderHook((props: LimitSwitchHost) => useLimitSwitch(props), { initialProps: start });
    act(() => result.current.switchTo({ runner: "codex", model: GPT, permission: "read-only" }, "fix it"));
    expect(start.onPermissionChange).not.toHaveBeenCalled();
    rerender({ ...start, currentRunner: "codex", selectedModel: null });
    expect(start.onPermissionChange).not.toHaveBeenCalled();
    rerender({ ...start, currentRunner: "codex", selectedModel: GPT });
    expect(start.onPermissionChange).toHaveBeenCalledWith("read-only");
    expect(start.onSend).not.toHaveBeenCalled();
    await act(async () => rerender({ ...start, currentRunner: "codex", selectedModel: GPT, selectedPermission: "read-only" }));
    expect(start.onSend).toHaveBeenCalledWith("fix it");
  });

  it("keeps the existing permission when no override was chosen", async () => {
    const start = host({ currentRunner: "codex", selectedPermission: "read-only", onPermissionChange: vi.fn() });
    const { result } = renderHook(() => useLimitSwitch(start));
    await act(async () => result.current.switchTo({ runner: "codex", model: null, permission: null }, "fix it"));
    expect(start.onPermissionChange).not.toHaveBeenCalled();
    expect(start.onSend).toHaveBeenCalledWith("fix it");
  });

  it("blocks a handoff if the selected model changes while permission is being applied", () => {
    const start = host({ currentRunner: "codex", selectedModel: GPT,
      selectedPermission: "default", onPermissionChange: vi.fn() });
    const { result, rerender } = renderHook((props: LimitSwitchHost) => useLimitSwitch(props), { initialProps: start });
    act(() => result.current.switchTo({ runner: "codex", model: GPT, permission: "read-only" }, "retry"));
    expect(start.onPermissionChange).toHaveBeenCalledWith("read-only");
    rerender({ ...start, selectedPermission: "read-only", selectedModel: { providerID: "openai", modelID: "luna" } });
    expect(start.onSend).not.toHaveBeenCalled();
    expect(result.current.error).toContain("Model changed before the handoff");
  });

  it("does not retry when the host cannot apply the chosen permission", () => {
    vi.useFakeTimers();
    try {
      const start = host({ currentRunner: "codex", selectedPermission: "default", onPermissionChange: vi.fn() });
      const { result } = renderHook(() => useLimitSwitch(start));
      act(() => result.current.switchTo({ runner: "codex", model: null, permission: "read-only" }, "fix it"));
      act(() => vi.advanceTimersByTime(MODEL_WAIT_MS));
      expect(start.onSend).not.toHaveBeenCalled();
      expect(result.current.switching).toBe(false);
      expect(result.current.error).toContain("Permission could not be applied");
    } finally {
      vi.useRealTimers();
    }
  });

  it("refuses a handoff when the composer keeps replacing the chosen model", () => {
    const start = host({ currentRunner: "codex", selectedModel: null });
    const { result, rerender } = renderHook((props: LimitSwitchHost) => useLimitSwitch(props), {
      initialProps: start,
    });
    act(() => result.current.switchTo({ runner: "codex", model: GPT }, "again"));
    expect(start.onModelSelected).toHaveBeenCalledTimes(1);
    // Renders with nothing new on screen do not count as a refusal.
    rerender({ ...start });
    expect(start.onModelSelected).toHaveBeenCalledTimes(1);
    // The composer's own repair picked something else each time.
    for (const modelID of ["a", "b", "c", "d"]) {
      rerender({ ...start, selectedModel: { providerID: "openai", modelID } });
    }
    expect(start.onModelSelected).toHaveBeenCalledTimes(3);
    expect(start.onSend).not.toHaveBeenCalled();
    expect(result.current.error).toContain("Model could not be applied");
  });

  it("does not hand off on a different model when the composer never reports it", () => {
    vi.useFakeTimers();
    try {
      const start = host({ currentRunner: "codex", selectedModel: null });
      const { result } = renderHook((props: LimitSwitchHost) => useLimitSwitch(props), {
        initialProps: start,
      });
      act(() => result.current.switchTo({ runner: "codex", model: GPT }, "again"));
      expect(start.onSend).not.toHaveBeenCalled();
      act(() => vi.advanceTimersByTime(MODEL_WAIT_MS));
      expect(start.onSend).not.toHaveBeenCalled();
      expect(result.current.error).toContain("Model could not be applied");
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps the switch busy until the handoff is accepted and reports failure", async () => {
    let finish: ((ok: boolean) => void) | undefined;
    const start = host({ currentRunner: "codex", onSend: vi.fn(() => new Promise<boolean>((resolve) => { finish = resolve; })) });
    const { result } = renderHook(() => useLimitSwitch(start));
    act(() => result.current.switchTo({ runner: "codex", model: null }, "retry"));
    expect(result.current.switching).toBe(true);
    act(() => result.current.switchTo({ runner: "codex", model: null }, "duplicate"));
    expect(start.onSend).toHaveBeenCalledTimes(1);
    await act(async () => finish?.(false));
    expect(result.current.switching).toBe(false);
    expect(result.current.error).toContain("Could not hand off");
  });

  it("shows a runner error from a rejected handoff", async () => {
    const start = host({ currentRunner: "codex", onSend: vi.fn().mockRejectedValue(new Error("Runner unavailable")) });
    const { result } = renderHook(() => useLimitSwitch(start));
    await act(async () => result.current.switchTo({ runner: "codex", model: null }));
    expect(result.current.error).toBe("Runner unavailable");
    expect(result.current.switching).toBe(false);
  });
});
