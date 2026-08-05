/**
 * Each runner keeps its own configuration.
 *
 * A model, an agent, an effort tier and a permission mode are all meaningful
 * only inside one runner, so going Claude → Codex → Claude must not cost the
 * user their Claude setup.
 */
import { describe, it, expect, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRunnerConfig, emptyConfig } from "../hooks/useRunnerConfig";

const haiku = { providerID: "anthropic", modelID: "claude-haiku-4-5" };
const gpt = { providerID: "openai", modelID: "gpt-5-mini" };

describe("useRunnerConfig", () => {
  beforeEach(() => localStorage.clear());

  it("defaults a runner it has never seen", () => {
    const { result } = renderHook(() => useRunnerConfig());
    expect(result.current.recall("claude")).toEqual(emptyConfig("claude"));
    // Codex has a different permission vocabulary, so its default differs.
    expect(result.current.recall("codex").permission).toBe("on-request");
  });

  it("keeps each runner's configuration apart", () => {
    const { result } = renderHook(() => useRunnerConfig());
    act(() => {
      result.current.remember("claude", { model: haiku, agent: "build", effort: "low" });
      result.current.remember("codex", { model: gpt, permission: "never" });
    });

    expect(result.current.recall("claude").model).toEqual(haiku);
    expect(result.current.recall("claude").agent).toBe("build");
    expect(result.current.recall("claude").effort).toBe("low");
    // Claude's settings did not leak into Codex, or the reverse.
    expect(result.current.recall("codex").model).toEqual(gpt);
    expect(result.current.recall("codex").agent).toBe("");
    expect(result.current.recall("codex").permission).toBe("never");
    expect(result.current.recall("claude").permission).toBe("default");
  });

  /* The UI clears the model and agent on session switches and runner changes.
     Recording those blanks would erase a good config as a side effect. */
  it("ignores cleared values", () => {
    const { result } = renderHook(() => useRunnerConfig());
    act(() => { result.current.remember("claude", { model: haiku, agent: "build" }); });
    act(() => { result.current.remember("claude", { model: null, agent: "" }); });

    expect(result.current.recall("claude").model).toEqual(haiku);
    expect(result.current.recall("claude").agent).toBe("build");
  });

  /* Effort is the exception: "default effort" is a real, chosen value. */
  it("records a null effort as a genuine choice", () => {
    const { result } = renderHook(() => useRunnerConfig());
    act(() => { result.current.remember("claude", { effort: "high" }); });
    expect(result.current.recall("claude").effort).toBe("high");
    act(() => { result.current.remember("claude", { effort: null }); });
    expect(result.current.recall("claude").effort).toBeNull();
  });

  it("survives a reload", () => {
    const first = renderHook(() => useRunnerConfig());
    act(() => { first.result.current.remember("claude", { model: haiku, effort: "medium" }); });
    first.unmount();

    const second = renderHook(() => useRunnerConfig());
    expect(second.result.current.recall("claude").model).toEqual(haiku);
    expect(second.result.current.recall("claude").effort).toBe("medium");
  });

  it("shrugs off corrupt storage", () => {
    localStorage.setItem("opman-runner-config", "{not json");
    const { result } = renderHook(() => useRunnerConfig());
    expect(result.current.recall("claude")).toEqual(emptyConfig("claude"));
    act(() => { result.current.remember("claude", { agent: "build" }); });
    expect(result.current.recall("claude").agent).toBe("build");
  });
});
