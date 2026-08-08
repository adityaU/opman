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
    expect(result.current.recall("claude")).toEqual(emptyConfig());
    // Every runner opens on the same permission: the ones with a real permission
    // model report their own modes, and that list replaces this value.
    expect(result.current.recall("codex")).toEqual(emptyConfig());
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
    expect(result.current.recall("claude")).toEqual(emptyConfig());
    expect(result.current.lastRunner()).toBe("");
    act(() => { result.current.remember("claude", { agent: "build" }); });
    expect(result.current.recall("claude").agent).toBe("build");
  });

  /* A value that is valid JSON but not an object at all — an array, a number —
     must be discarded rather than indexed into. */
  it("shrugs off storage of the wrong type", () => {
    for (const junk of ["[1,2]", "7", '"claude"', "null"]) {
      localStorage.setItem("opman-runner-config", junk);
      const { result, unmount } = renderHook(() => useRunnerConfig());
      expect(result.current.lastRunner()).toBe("");
      expect(result.current.recall("claude")).toEqual(emptyConfig());
      unmount();
    }
  });

  /* The runner *choice* is what a brand-new session opens on, so it has to
     outlive the tab the same way the per-runner configs do. */
  it("round-trips the last picked runner", () => {
    const first = renderHook(() => useRunnerConfig());
    expect(first.result.current.lastRunner()).toBe("");
    act(() => {
      first.result.current.rememberRunner("codex");
      first.result.current.remember("codex", { model: gpt, effort: "high" });
    });
    expect(first.result.current.lastRunner()).toBe("codex");
    first.unmount();

    const second = renderHook(() => useRunnerConfig());
    expect(second.result.current.lastRunner()).toBe("codex");
    // The per-runner map still reads back alongside it.
    expect(second.result.current.recall("codex").model).toEqual(gpt);
    expect(second.result.current.recall("codex").effort).toBe("high");
  });

  it("ignores an empty runner pick", () => {
    const { result } = renderHook(() => useRunnerConfig());
    act(() => { result.current.rememberRunner("claude"); });
    act(() => { result.current.rememberRunner(""); });
    expect(result.current.lastRunner()).toBe("claude");
  });

  /* v1 stored the per-runner map at the top level; those values are still in
     users' browsers and must survive the move under `runners`. */
  it("migrates the old flat shape", () => {
    localStorage.setItem(
      "opman-runner-config",
      JSON.stringify({ claude: { model: haiku, agent: "build", effort: "low", permission: "default" } }),
    );
    const { result } = renderHook(() => useRunnerConfig());
    expect(result.current.recall("claude").model).toEqual(haiku);
    expect(result.current.recall("claude").agent).toBe("build");
    // There was no recorded pick in v1, so there is nothing to prefer yet.
    expect(result.current.lastRunner()).toBe("");

    act(() => { result.current.rememberRunner("codex"); });
    const stored = JSON.parse(localStorage.getItem("opman-runner-config")!);
    expect(stored.lastRunner).toBe("codex");
    // Rewritten in the new shape without losing the migrated config.
    expect(stored.runners.claude.model).toEqual(haiku);
  });
});
