/**
 * Switching runner, then model, then re-sending: each step must wait until the composer
 * shows the previous one, or the send goes out on the old engine.
 */
import { describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { MODEL_WAIT_MS, useLimitSwitch, type LimitSwitchHost } from "../runner-limit/useLimitSwitch";

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
  it("changes runner, then model, then re-sends — each after the last took effect", () => {
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
    rerender({ ...start, currentRunner: "codex", selectedModel: GPT });
    expect(start.onSend).toHaveBeenCalledWith("fix the bug");
    expect(result.current.switching).toBe(false);
  });

  it("sends nothing when there is no prompt to retry", () => {
    const start = host();
    const { result, rerender } = renderHook((props: LimitSwitchHost) => useLimitSwitch(props), {
      initialProps: start,
    });
    act(() => result.current.switchTo({ runner: "codex", model: null }));
    rerender({ ...start, currentRunner: "codex", selectedModel: null });
    expect(start.onModelSelected).not.toHaveBeenCalled();
    expect(start.onSend).not.toHaveBeenCalled();
    expect(result.current.switching).toBe(false);
  });

  it("re-applies the model when the composer swaps in another, a few times at most", () => {
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
    expect(start.onSend).toHaveBeenCalledWith("again");
  });

  it("carries on without the model when the composer never reports it", () => {
    vi.useFakeTimers();
    try {
      const start = host({ currentRunner: "codex", selectedModel: null });
      const { result } = renderHook((props: LimitSwitchHost) => useLimitSwitch(props), {
        initialProps: start,
      });
      act(() => result.current.switchTo({ runner: "codex", model: GPT }, "again"));
      expect(start.onSend).not.toHaveBeenCalled();
      act(() => vi.advanceTimersByTime(MODEL_WAIT_MS));
      expect(start.onSend).toHaveBeenCalledWith("again");
    } finally {
      vi.useRealTimers();
    }
  });
});
