/**
 * A catalogue belongs to one runner. Switching runners must never show, or keep, the
 * previous runner's models — "claude models for codex and vice versa".
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

const mockFetchProviders = vi.fn();
const mockFetchAgents = vi.fn();
vi.mock("../api", () => ({
  fetchProviders: (...args: unknown[]) => mockFetchProviders(...args),
  fetchAgents: (...args: unknown[]) => mockFetchAgents(...args),
}));

import { useProviders, invalidateProviderCache } from "../hooks/useProviders";
import { useAgents } from "../hooks/useAgents";
import { useEngineOptions } from "../engine-picker/useEngineOptions";

function catalogue(runner: string) {
  return {
    all: [{ id: runner, name: runner, models: { [`${runner}-model`]: { name: runner } } }],
    connected: [runner],
    default: { [runner]: `${runner}-model` },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => { resolve = r; });
  return { promise, resolve };
}

const idsOf = (all: { id: string }[]) => all.map((p) => p.id);

beforeEach(() => {
  invalidateProviderCache();
  mockFetchProviders.mockReset();
  mockFetchAgents.mockReset();
});

describe("useProviders across a runner switch", () => {
  it("a slow answer for the previous runner does not overwrite the current one", async () => {
    // The ACP /provider read can wait up to 30s for the startup probe, so the claude
    // answer routinely lands after the user has already moved on to codex.
    const claude = deferred<ReturnType<typeof catalogue>>();
    const codex = deferred<ReturnType<typeof catalogue>>();
    mockFetchProviders.mockImplementation((runner: string) =>
      runner === "claude" ? claude.promise : codex.promise);

    const { result, rerender } = renderHook(({ runner }) => useProviders(runner), {
      initialProps: { runner: "claude" },
    });
    rerender({ runner: "codex" });

    await act(async () => { codex.resolve(catalogue("codex")); });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(idsOf(result.current.all)).toEqual(["codex"]);

    await act(async () => { claude.resolve(catalogue("claude")); });
    expect(idsOf(result.current.all)).toEqual(["codex"]);
    expect(result.current.defaults).toEqual({ codex: "codex-model" });
    expect([...result.current.connected]).toEqual(["codex"]);
  });

  it("the late answer still fills its own runner's cache", async () => {
    const claude = deferred<ReturnType<typeof catalogue>>();
    mockFetchProviders.mockImplementation((runner: string) =>
      runner === "claude" ? claude.promise : Promise.resolve(catalogue("codex")));

    const { result, rerender } = renderHook(({ runner }) => useProviders(runner), {
      initialProps: { runner: "claude" },
    });
    rerender({ runner: "codex" });
    await act(async () => { claude.resolve(catalogue("claude")); });

    mockFetchProviders.mockClear();
    rerender({ runner: "claude" });
    expect(idsOf(result.current.all)).toEqual(["claude"]);
    expect(mockFetchProviders).not.toHaveBeenCalled();
  });

  it("does not show the previous runner's catalogue while the new one loads", async () => {
    mockFetchProviders.mockImplementation((runner: string) =>
      runner === "claude" ? Promise.resolve(catalogue("claude")) : new Promise(() => {}));

    const { result, rerender } = renderHook(({ runner }) => useProviders(runner), {
      initialProps: { runner: "claude" },
    });
    await waitFor(() => expect(idsOf(result.current.all)).toEqual(["claude"]));

    rerender({ runner: "codex" });
    expect(result.current.all).toEqual([]);
    expect(result.current.loading).toBe(true);
    expect(result.current.defaults).toEqual({});
    expect(result.current.connected.size).toBe(0);
  });

  it("switching back to a cached runner shows its own catalogue at once", async () => {
    mockFetchProviders.mockImplementation((runner: string) => Promise.resolve(catalogue(runner)));

    const { result, rerender } = renderHook(({ runner }) => useProviders(runner), {
      initialProps: { runner: "claude" },
    });
    await waitFor(() => expect(idsOf(result.current.all)).toEqual(["claude"]));
    rerender({ runner: "codex" });
    await waitFor(() => expect(idsOf(result.current.all)).toEqual(["codex"]));

    rerender({ runner: "claude" });
    expect(idsOf(result.current.all)).toEqual(["claude"]);
    expect(result.current.loading).toBe(false);
  });
});

describe("useAgents across a runner switch", () => {
  it("does not show the previous runner's agents while the new one loads", async () => {
    mockFetchAgents.mockImplementation((runner: string) =>
      runner === "claude-a"
        ? Promise.resolve([{ id: "claude-agent", name: "Claude agent" }])
        : new Promise(() => {}));

    const { result, rerender } = renderHook(({ runner }) => useAgents(runner), {
      initialProps: { runner: "claude-a" },
    });
    await waitFor(() => expect(result.current.agents.map((a) => a.id)).toEqual(["claude-agent"]));

    rerender({ runner: "codex-a" });
    expect(result.current.agents).toEqual([]);
    expect(result.current.loading).toBe(true);
  });
});

describe("useEngineOptions across a runner switch", () => {
  it("never repairs the new runner's model to one of the previous runner's", async () => {
    const claude = deferred<ReturnType<typeof catalogue>>();
    const codex = deferred<ReturnType<typeof catalogue>>();
    mockFetchProviders.mockImplementation((runner: string) =>
      runner === "claude" ? claude.promise : codex.promise);
    mockFetchAgents.mockResolvedValue([]);
    const onModelSelected = vi.fn();
    const noop = () => {};

    const { rerender } = renderHook(
      ({ runner, model }) => useEngineOptions(runner, model, "", onModelSelected, noop),
      { initialProps: { runner: "claude", model: { providerID: "claude", modelID: "claude-model" } } },
    );
    rerender({ runner: "codex", model: { providerID: "codex", modelID: "codex-model" } });
    await act(async () => { codex.resolve(catalogue("codex")); });
    await act(async () => { claude.resolve(catalogue("claude")); });

    expect(onModelSelected).not.toHaveBeenCalled();
  });
});
