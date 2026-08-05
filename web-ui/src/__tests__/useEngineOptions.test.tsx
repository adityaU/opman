/**
 * Runner ownership: a model or agent that the current runner does not offer is
 * not a valid selection, and must be repaired rather than displayed.
 *
 * This is the failure the merged picker exists to prevent — the old controls
 * refetched the lists per runner but left the *selection* pointing at whatever
 * the previous runner had, so the composer could claim a model its runner has
 * never heard of.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useEngineOptions } from "../engine-picker/useEngineOptions";

const anthropic = {
  id: "anthropic",
  name: "Anthropic",
  models: { "claude-haiku-4-5": { id: "claude-haiku-4-5", name: "Haiku 4.5" } },
};
const openai = {
  id: "openai",
  name: "OpenAI",
  models: { "gpt-5-mini": { id: "gpt-5-mini", name: "GPT-5 mini" } },
};

const providersByRunner: Record<string, any> = {
  claude: { all: [anthropic], connected: new Set(["anthropic"]), defaults: { anthropic: "claude-haiku-4-5" } },
  codex: { all: [openai], connected: new Set(["openai"]), defaults: { openai: "gpt-5-mini" } },
};

vi.mock("../hooks/useProviders", () => ({
  useProviders: (runner: string) => ({
    ...(providersByRunner[runner] ?? { all: [], connected: new Set(), defaults: {} }),
    loading: false,
    error: null,
    refresh: vi.fn(),
  }),
}));

const agentsByRunner: Record<string, any[]> = {
  claude: [{ id: "build", label: "Build", description: "" }],
  codex: [{ id: "codex-default", label: "Codex", description: "" }],
};

vi.mock("../api", () => ({
  fetchAgents: (runner: string) => Promise.resolve(agentsByRunner[runner] ?? []),
}));

describe("useEngineOptions", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists only the runner's own models and agents", async () => {
    const { result } = renderHook(() =>
      useEngineOptions("claude", { providerID: "anthropic", modelID: "claude-haiku-4-5" }, "build", vi.fn(), vi.fn()),
    );

    await waitFor(() => expect(result.current.agents).toHaveLength(1));
    expect(result.current.models.map((m) => m.modelId)).toEqual(["claude-haiku-4-5"]);
    expect(result.current.agents[0].id).toBe("build");
  });

  it("leaves a still-valid selection alone", async () => {
    const onModel = vi.fn();
    const onAgent = vi.fn();
    renderHook(() =>
      useEngineOptions("claude", { providerID: "anthropic", modelID: "claude-haiku-4-5" }, "build", onModel, onAgent),
    );

    await waitFor(() => expect(onAgent).not.toHaveBeenCalled());
    expect(onModel).not.toHaveBeenCalled();
  });

  it("repairs a model the new runner does not offer", async () => {
    const onModel = vi.fn();
    // Carrying Anthropic's model into the Codex runner: nothing in Codex's list
    // matches, so it falls back to that runner's default.
    renderHook(() =>
      useEngineOptions("codex", { providerID: "anthropic", modelID: "claude-haiku-4-5" }, "codex-default", onModel, vi.fn()),
    );

    await waitFor(() => expect(onModel).toHaveBeenCalledWith("gpt-5-mini", "openai"));
  });

  it("repairs an agent the new runner does not offer", async () => {
    const onAgent = vi.fn();
    renderHook(() =>
      useEngineOptions("codex", { providerID: "openai", modelID: "gpt-5-mini" }, "build", vi.fn(), onAgent),
    );

    await waitFor(() => expect(onAgent).toHaveBeenCalledWith("codex-default"));
  });

  it("fills an empty model selection from the runner's default", async () => {
    const onModel = vi.fn();
    renderHook(() => useEngineOptions("claude", null, "build", onModel, vi.fn()));

    await waitFor(() => expect(onModel).toHaveBeenCalledWith("claude-haiku-4-5", "anthropic"));
  });
});
