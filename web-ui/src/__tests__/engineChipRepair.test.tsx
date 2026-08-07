/**
 * The repair has to run while the picker is CLOSED.
 *
 * `useEngineOptions` has always known how to reconcile a model with its
 * runner, but it was mounted inside `EnginePalette` — a component that only
 * exists while the palette is open. So the chip could sit there advertising
 * `Claude · zen/big-pickle` indefinitely, and the repair would fire only when
 * the user opened the one surface that would have shown them the right value
 * anyway. These tests pin the fix at the level the bug appeared: the chip.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor, screen } from "@testing-library/react";
import { EngineChip } from "../engine-picker/EngineChip";

const anthropic = {
  id: "anthropic",
  name: "Anthropic",
  models: { "claude-haiku-4-5": { id: "claude-haiku-4-5", name: "Haiku 4.5" } },
};

vi.mock("../hooks/useProviders", () => ({
  useProviders: () => ({
    all: [anthropic],
    connected: new Set(["anthropic"]),
    defaults: { anthropic: "claude-haiku-4-5" },
    permissionModes: null,
    loading: false,
    error: null,
    refresh: vi.fn(),
  }),
}));

vi.mock("../hooks/useAgents", () => ({
  useAgents: () => ({ agents: [{ id: "build", label: "Build", description: "" }], loading: false }),
}));

function renderChip(over: Partial<React.ComponentProps<typeof EngineChip>> = {}) {
  const onModelSelected = vi.fn();
  const onAgentChange = vi.fn();
  render(
    <EngineChip
      runner="claude"
      availableRunners={["claude", "codex"]}
      currentModel="zen/big-pickle"
      selectedModel={{ providerID: "zen", modelID: "big-pickle" }}
      currentAgent="build"
      agents={[{ id: "build", label: "Build" } as any]}
      disabled={false}
      supportedEfforts={[]}
      effort={null}
      permission="default"
      onRunnerChange={vi.fn()}
      onModelSelected={onModelSelected}
      onAgentChange={onAgentChange}
      onEffortChange={vi.fn()}
      onPermissionChange={vi.fn()}
      {...over}
    />,
  );
  return { onModelSelected, onAgentChange };
}

describe("EngineChip keeps runner and model consistent", () => {
  beforeEach(() => vi.clearAllMocks());

  it("repairs a model the runner does not offer, without the palette being opened", () => {
    const { onModelSelected } = renderChip();
    // No click: the palette is never mounted in this test.
    expect(screen.queryByRole("dialog")).toBeNull();
    return waitFor(() =>
      expect(onModelSelected).toHaveBeenCalledWith("claude-haiku-4-5", "anthropic"),
    );
  });

  it("leaves a model the runner does offer alone", async () => {
    const { onModelSelected } = renderChip({
      currentModel: "anthropic/claude-haiku-4-5",
      selectedModel: { providerID: "anthropic", modelID: "claude-haiku-4-5" },
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(onModelSelected).not.toHaveBeenCalled();
  });

  it("repairs an agent the runner does not offer", () => {
    const { onAgentChange } = renderChip({ currentAgent: "codex-default" });
    return waitFor(() => expect(onAgentChange).toHaveBeenCalledWith("build"));
  });
});
