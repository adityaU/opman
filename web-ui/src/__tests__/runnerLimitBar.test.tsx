/**
 * The composer strip: "ask" offers the best target and switches on click, "auto" says
 * where the server will move the chat (and offers, like "ask", when nothing is listed),
 * and "off" stays silent.
 */
import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MODEL_WAIT_MS } from "../runner-limit/useLimitSwitch";
import { RunnerLimitBar, type RunnerLimitBarProps } from "../runner-limit/RunnerLimitBar";
import { DISMISS_KEY } from "../runner-limit/dismissals";
import { resetSettingsStore } from "../runner-limit/settingsStore";
import type { RunnerLimitSettings } from "../api/runnerLimits";
import type { UsageReport } from "../usage/usageClient";

const RESET = new Date(Date.now() + 3 * 3_600_000).toISOString();
const REPORT: UsageReport = {
  accounts: [
    { id: "anthropic", label: "Claude", state: "ok", plan: null, credits: null, note: null, stale: false, fetchedAt: null,
      windows: [{ id: "5h", label: "5-hour", usedPercent: 96, resetsAt: RESET, severity: null }] },
    { id: "openai", label: "Codex", state: "ok", plan: null, credits: null, note: null, stale: false, fetchedAt: null,
      windows: [{ id: "5h", label: "5-hour", usedPercent: 30, resetsAt: RESET, severity: null }] },
  ],
  runners: { claude: "anthropic", codex: "openai" },
};

// The settings come from the server; these tests set the store directly instead.
vi.mock("../api/runnerLimits", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("../api/runnerLimits");
  return { ...actual, fetchRunnerLimits: vi.fn(() => new Promise(() => {})), saveRunnerLimits: vi.fn() };
});

function settings(change: Partial<RunnerLimitSettings>): void {
  resetSettingsStore({ policy: "ask", nearPercent: 5, pairs: [], ...change });
}

vi.mock("../usage/useUsage", () => ({
  useUsage: () => ({ report: REPORT, status: "ready", error: null, loadedAt: 0, refreshing: false, refresh: vi.fn() }),
}));

const catalogues = vi.hoisted(() => ({
  modes: {
    codex: [{ value: "read-only", label: "Read only" }, { value: "workspace-write", label: "Workspace write" }],
  } as Record<string, { value: string; label: string }[]>,
}));
vi.mock("../hooks/useProviders", () => ({
  useProviders: (runner: string) => ({ permissionModes: catalogues.modes[runner] ?? null, loading: false }),
}));

function props(overrides: Partial<RunnerLimitBarProps> = {}): RunnerLimitBarProps {
  return {
    sessionId: "s1",
    runner: "claude",
    availableRunners: ["claude", "codex"],
    selectedModel: null,
    selectedPermission: "default",
    isBusy: false,
    onRunnerChange: vi.fn(),
    onModelSelected: vi.fn(),
    onPermissionChange: vi.fn(),
    onSend: vi.fn().mockResolvedValue(true),
    ...overrides,
  };
}

beforeEach(() => {
  localStorage.removeItem(DISMISS_KEY);
  settings({});
});
afterEach(cleanup);

describe("RunnerLimitBar", () => {
  it("asks first: names what is left and switches on click", () => {
    const p = props();
    render(<RunnerLimitBar {...p} />);
    expect(screen.getByText("Claude has 4% left")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Switch" }));
    expect(p.onRunnerChange).toHaveBeenCalledWith("codex");
  });

  it("stays hidden once dismissed", () => {
    render(<RunnerLimitBar {...props()} />);
    fireEvent.click(screen.getByRole("button", { name: "Not now" }));
    expect(screen.queryByText("Claude has 4% left")).toBeNull();
  });

  it("offers, rather than takes, a runner the user never listed", () => {
    settings({ policy: "auto" });
    const p = props();
    render(<RunnerLimitBar {...p} />);
    expect(p.onRunnerChange).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Switch" })).toBeTruthy();
  });

  it("in auto mode says where the server will move the chat, and moves nothing itself", () => {
    settings({
      policy: "auto",
      pairs: [{ runner: "codex", model: { providerID: "openai", modelID: "gpt-5.6-sol" } }],
    });
    const p = props();
    render(<RunnerLimitBar {...p} />);
    expect(screen.getByText(/Your next message goes to Codex · gpt-5.6-sol/)).toBeTruthy();
    expect(p.onRunnerChange).not.toHaveBeenCalled();
    expect(p.onSend).not.toHaveBeenCalled();
  });

  it("is silent when switched off", () => {
    settings({ policy: "off" });
    const { container } = render(<RunnerLimitBar {...props()} />);
    expect(container.textContent).toBe("");
  });

  it("uses the saved target permission and lets the user change it before switching", () => {
    settings({ pairs: [{ runner: "codex", model: null, permission: "workspace-write" }] });
    const p = props();
    const view = render(<RunnerLimitBar {...p} />);
    const permission = screen.getByRole("combobox", { name: "Handoff permission" }) as HTMLSelectElement;
    expect(permission.value).toBe("workspace-write");
    fireEvent.change(permission, { target: { value: "read-only" } });
    fireEvent.click(screen.getByRole("button", { name: "Switch" }));
    expect(p.onPermissionChange).not.toHaveBeenCalled();
    view.rerender(<RunnerLimitBar {...p} runner="codex" />);
    expect(p.onPermissionChange).toHaveBeenCalledWith("read-only");
    expect(p.onSend).not.toHaveBeenCalled();
  });

  it("shows only the chosen runner's reported permissions", () => {
    render(<RunnerLimitBar {...props({ availableRunners: ["claude", "codex", "opencode"] })} />);
    expect(screen.getByRole("option", { name: "Read only" })).toBeTruthy();
    fireEvent.change(screen.getByRole("combobox", { name: "Move this chat to" }), { target: { value: "opencode" } });
    expect(screen.queryByRole("combobox", { name: "Handoff permission" })).toBeNull();
  });

  it("requires a new choice when a saved permission is no longer reported", () => {
    settings({ pairs: [{ runner: "codex", model: null, permission: "removed-mode" }] });
    render(<RunnerLimitBar {...props()} />);
    const switchButton = screen.getByRole("button", { name: "Switch" }) as HTMLButtonElement;
    expect(switchButton.disabled).toBe(true);
    fireEvent.change(screen.getByRole("combobox", { name: "Handoff permission" }), { target: { value: "read-only" } });
    expect(switchButton.disabled).toBe(false);
  });

  it("shows a failed permission change after moving to a healthy runner", () => {
    vi.useFakeTimers();
    try {
      settings({ pairs: [{ runner: "codex", model: null, permission: "read-only" }] });
      const p = props();
      const view = render(<RunnerLimitBar {...p} />);
      fireEvent.click(screen.getByRole("button", { name: "Switch" }));
      view.rerender(<RunnerLimitBar {...p} runner="codex" />);
      act(() => vi.advanceTimersByTime(MODEL_WAIT_MS));
      expect(screen.getByRole("alert").textContent).toContain("Permission could not be applied");
      expect(p.onSend).not.toHaveBeenCalled();
      fireEvent.click(screen.getByRole("button", { name: "Hide" }));
      expect(screen.queryByRole("alert")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});
