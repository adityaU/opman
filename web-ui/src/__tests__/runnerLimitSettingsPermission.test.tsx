import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { FallbackAdder } from "../settings-page/limits/FallbackAdder";
import { RunnerLimitsSection } from "../settings-page/limits/RunnerLimitsSection";
import { getSettings, resetSettingsStore } from "../runner-limit/settingsStore";
import type { FallbackPair } from "../api/runnerLimits";

const { save, providerCache } = vi.hoisted(() => ({ save: vi.fn(), providerCache: vi.fn() }));

vi.mock("../hooks/useProviders", () => ({ useProviders: providerCache }));
vi.mock("../api/runnerLimits", async () => ({
  ...await vi.importActual<Record<string, unknown>>("../api/runnerLimits"),
  fetchRunnerLimits: vi.fn(() => new Promise(() => {})),
  saveRunnerLimits: save,
}));

beforeEach(() => {
  save.mockImplementation((settings) => Promise.resolve(settings));
  providerCache.mockImplementation((runner: string) => ({
    all: [], connected: new Set(), defaults: {}, loading: false, error: null,
    permissionModes: runner === "codex"
      ? [{ value: "read-only", label: "Read only" }, { value: "workspace-write", label: "Workspace write" }]
      : null,
    refresh: vi.fn(),
  }));
  resetSettingsStore({ policy: "auto", nearPercent: 5, pairs: [] });
});

afterEach(async () => {
  await act(async () => {});
  cleanup();
  vi.clearAllMocks();
});

describe("usage limit settings permissions", () => {
  it("adds a runner with its selected permission and resets when changing runner", () => {
    const add = vi.fn();
    render(<FallbackAdder runners={["codex", "claude"]} onAdd={add} />);
    fireEvent.change(screen.getByLabelText("Permission for new fallback"), {
      target: { value: "read-only" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(add).toHaveBeenLastCalledWith({ runner: "codex", model: null, permission: "read-only" });
    fireEvent.change(screen.getByLabelText("Permission for new fallback"), {
      target: { value: "workspace-write" },
    });
    fireEvent.change(screen.getByLabelText("Runner"), { target: { value: "claude" } });
    expect(screen.queryByRole("option", { name: "Read only" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(add).toHaveBeenLastCalledWith({ runner: "claude", model: null, permission: null });
  });

  it("saves changes to an existing fallback and can return to compatible permission", async () => {
    const pair: FallbackPair = { runner: "codex", model: null, permission: "read-only" };
    resetSettingsStore({ policy: "auto", nearPercent: 5, pairs: [pair] });
    render(<RunnerLimitsSection runners={["codex"]} />);
    const select = screen.getByLabelText("Permission for fallback 1, Codex") as HTMLSelectElement;
    expect(select.value).toBe("read-only");
    await act(async () => {
      fireEvent.change(select, { target: { value: "workspace-write" } });
    });
    expect(save).toHaveBeenLastCalledWith(expect.objectContaining({
      pairs: [{ ...pair, permission: "workspace-write" }],
    }));
    await act(async () => {
      fireEvent.change(select, { target: { value: "" } });
    });
    expect(getSettings().pairs[0].permission).toBeNull();
  });

  it("updates an existing runner and model instead of silently discarding a new permission", () => {
    resetSettingsStore({ policy: "auto", nearPercent: 5, pairs: [{ runner: "codex", model: null }] });
    render(<RunnerLimitsSection runners={["codex"]} />);
    fireEvent.change(screen.getByLabelText("Permission for new fallback"), {
      target: { value: "read-only" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(getSettings().pairs).toEqual([{ runner: "codex", model: null, permission: "read-only" }]);
  });

  it("keeps an unavailable saved mode visible until the user clears it", () => {
    resetSettingsStore({ policy: "auto", nearPercent: 5,
      pairs: [{ runner: "claude", model: null, permission: "old-mode" }] });
    render(<RunnerLimitsSection runners={["claude"]} />);
    const select = screen.getByLabelText("Permission for fallback 1, Claude") as HTMLSelectElement;
    expect(select.value).toBe("old-mode");
    expect(screen.getByRole("option", { name: "Saved mode unavailable" }).hasAttribute("disabled")).toBe(true);
    fireEvent.change(select, { target: { value: "" } });
    expect(getSettings().pairs[0].permission).toBeNull();
  });
});
