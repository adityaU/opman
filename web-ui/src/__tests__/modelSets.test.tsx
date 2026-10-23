import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ModelSetsSection } from "../settings-page/limits/ModelSetsSection";
import { ModelSetEditor } from "../settings-page/limits/ModelSetEditor";
import { getSettings, resetSettingsStore } from "../runner-limit/settingsStore";
import { limitOptions } from "../runner-limit/chooseTarget";
import type { FallbackPair, ModelSet } from "../api/runnerLimits";

const save = vi.hoisted(() => vi.fn());
vi.mock("../api/runnerLimits", async () => ({
  ...await vi.importActual<Record<string, unknown>>("../api/runnerLimits"),
  saveRunnerLimits: save, fetchRunnerLimits: vi.fn(() => new Promise(() => {})),
}));
vi.mock("../hooks/useProviders", () => ({
  useProviders: (runner: string) => ({
    all: [{ id: runner, name: runner, models: runner === "claude"
      ? { opus: { name: "Opus" }, sonnet: { name: "Sonnet" } }
      : { sol: { name: "Sol" }, luna: { name: "Luna" } } }],
    connected: new Set([runner]), loading: false,
    permissionModes: [{ value: "read-only", label: "Read only" }],
  }),
}));

const member = (runner: string, modelID: string): FallbackPair =>
  ({ runner, model: { providerID: runner, modelID }, permission: null });
const sets: ModelSet[] = [
  { id: "deep", name: "Deep work", members: [member("claude", "opus"), member("codex", "sol")] },
  { id: "fast", name: "Quick work", members: [member("claude", "sonnet"), member("codex", "luna")] },
];
const input = {
  report: null, current: "claude", level: "out" as const, pairs: [],
  available: ["claude", "codex", "opencode"], now: Date.now(), nearPercent: 5, modelSets: sets,
};
beforeEach(() => {
  save.mockImplementation(settings => Promise.resolve(settings));
  resetSettingsStore({ policy: "ask", pairs: [], nearPercent: 5, modelSets: [] });
});
afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("model set selection", () => {
  it("maps Opus to Sol and Sonnet to Luna, preserving permissions", () => {
    expect(limitOptions({ ...input, currentModel: member("claude", "opus").model }))
      .toEqual([{ ...member("codex", "sol"), source: "preselected", leftPercent: null }]);
    expect(limitOptions({ ...input, currentModel: member("claude", "sonnet").model })[0].model?.modelID)
      .toBe("luna");
  });
  it("never leaves a matching set when its other runners are unavailable", () => {
    expect(limitOptions({ ...input, available: ["claude", "opencode"],
      currentModel: member("claude", "opus").model })).toEqual([]);
  });
  it("uses legacy fallbacks when the current model has no set", () => {
    expect(limitOptions({ ...input, pairs: [member("opencode", "other")],
      currentModel: member("claude", "unmatched").model })[0].runner).toBe("opencode");
  });
  it("does not match a model id from another provider", () => {
    expect(limitOptions({ ...input, currentModel: { providerID: "other", modelID: "opus" } })[0].source)
      .toBe("suggested");
  });
});

describe("model set Settings", () => {
  it("creates a named set from real model choices and saves both members", async () => {
    render(<ModelSetsSection runners={["claude", "codex"]} />);
    fireEvent.click(screen.getByRole("button", { name: "New model set" }));
    fireEvent.change(screen.getByLabelText("Set name"), { target: { value: "Deep work" } });
    expect(screen.getByRole("button", { name: "Save set" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Model"), { target: { value: "claude/opus" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    fireEvent.change(screen.getByLabelText("Runner"), { target: { value: "codex" } });
    fireEvent.change(screen.getByLabelText("Model"), { target: { value: "codex/sol" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Save set" })); });
    expect(getSettings().modelSets?.[0]).toMatchObject({ name: "Deep work", members: sets[0].members });
    expect(screen.queryByLabelText("Set name")).toBeNull();
  });
  it("edits, reorders and deletes saved sets", async () => {
    resetSettingsStore({ policy: "auto", pairs: [], nearPercent: 5, modelSets: sets });
    render(<ModelSetsSection runners={["claude", "codex"]} />);
    fireEvent.click(screen.getByLabelText("Edit Deep work"));
    fireEvent.change(screen.getByLabelText("Set name"), { target: { value: "Careful work" } });
    fireEvent.click(screen.getByLabelText("Move Codex up"));
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Save set" })); });
    expect(getSettings().modelSets?.[0].members[0].runner).toBe("codex");
    await act(async () => { fireEvent.click(screen.getByLabelText("Delete Careful work")); });
    expect(getSettings().modelSets?.map(set => set.id)).toEqual(["fast"]);
  });
  it("keeps an unsaved draft and shows failed saves", async () => {
    save.mockRejectedValue(new Error("Disk is full"));
    render(<ModelSetEditor initial={sets[0]} sets={sets} runners={["claude", "codex"]}
      onSave={async () => { throw new Error("Disk is full"); }} onCancel={vi.fn()} />);
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Save set" })); });
    expect(screen.getByRole("alert")).toHaveTextContent("Disk is full");
    expect(screen.getByLabelText("Set name")).toHaveValue("Deep work");
  });
  it("prevents a model from belonging to two sets", () => {
    render(<ModelSetEditor initial={{ id: "third", name: "", members: [] }} sets={sets}
      runners={["claude", "codex"]} onSave={vi.fn()} onCancel={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Model"), { target: { value: "claude/opus" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Deep work");
    expect(screen.getByRole("list", { name: "Models in this set" }).children).toHaveLength(0);
  });
});
