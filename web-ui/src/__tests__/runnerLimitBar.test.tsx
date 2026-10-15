/**
 * The composer strip: "ask" offers the best target and switches on click, "auto" says
 * where the server will move the chat (and offers, like "ask", when nothing is listed),
 * and "off" stays silent.
 */
import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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

function props(overrides: Partial<RunnerLimitBarProps> = {}): RunnerLimitBarProps {
  return {
    sessionId: "s1",
    runner: "claude",
    availableRunners: ["claude", "codex"],
    selectedModel: null,
    isBusy: false,
    onRunnerChange: vi.fn(),
    onModelSelected: vi.fn(),
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
});
