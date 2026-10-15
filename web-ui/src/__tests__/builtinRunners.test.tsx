/**
 * Settings → Agents: switching OpenCode / Claude Code on and off and choosing the default
 * runner; and a send to a switched-off runner says so instead of a bare failure.
 */
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ApiError } from "../api/client";

const fetchRunnerSettings = vi.fn();
const setRunnerEnabled = vi.fn();
const setDefaultRunner = vi.fn();
vi.mock("../api/runnerSettings", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("../api/runnerSettings");
  return {
    ...actual,
    fetchRunnerSettings: (...args: unknown[]) => fetchRunnerSettings(...args),
    setRunnerEnabled: (...args: unknown[]) => setRunnerEnabled(...args),
    setDefaultRunner: (...args: unknown[]) => setDefaultRunner(...args),
  };
});

import { BuiltinRunnersCard } from "../settings-page/acp/BuiltinRunnersCard";
import { sendFailureMessage } from "../api/runnerSettings";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const view = (opencodeOn: boolean) => ({
  builtin: [
    { id: "opencode", name: "OpenCode", installed: true, enabled: opencodeOn, primary: true },
    { id: "claude-code", name: "Claude Code", installed: false, enabled: true, primary: false },
  ],
  default: opencodeOn ? "opencode" : "claude",
  choices: opencodeOn ? ["claude", "opencode"] : ["claude"],
});

describe("BuiltinRunnersCard", () => {
  it("switches a runner off and shows where new sessions now go", async () => {
    fetchRunnerSettings.mockResolvedValue(view(true));
    setRunnerEnabled.mockResolvedValue(view(false));
    render(<BuiltinRunnersCard onError={vi.fn()} />);
    fireEvent.click(await screen.findByRole("switch", { name: "Turn off OpenCode" }));
    expect(setRunnerEnabled).toHaveBeenCalledWith("opencode", false);
    expect(await screen.findByRole("switch", { name: "Turn on OpenCode" })).toBeTruthy();
    expect((screen.getByRole("combobox") as HTMLSelectElement).value).toBe("claude");
  });

  it("cannot switch a runner that is not installed", async () => {
    fetchRunnerSettings.mockResolvedValue(view(true));
    render(<BuiltinRunnersCard onError={vi.fn()} />);
    const claude = await screen.findByRole("switch", { name: "Turn off Claude Code" });
    expect((claude as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText("not installed")).toBeTruthy();
  });

  it("changes the default runner", async () => {
    fetchRunnerSettings.mockResolvedValue(view(true));
    setDefaultRunner.mockResolvedValue({ ...view(true), default: "claude" });
    render(<BuiltinRunnersCard onError={vi.fn()} />);
    const select = (await screen.findByRole("combobox")) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "claude" } });
    expect(setDefaultRunner).toHaveBeenCalledWith("claude");
    await waitFor(() => expect(select.value).toBe("claude"));
  });
});

describe("sendFailureMessage", () => {
  it("passes on the turned-off reason and keeps the fallback otherwise", () => {
    const off = new ApiError("The opencode runner is turned off.", { code: "runner_disabled" });
    expect(sendFailureMessage(off, "Failed")).toBe("The opencode runner is turned off.");
    expect(sendFailureMessage(new ApiError("boom"), "Failed")).toBe("Failed");
    expect(sendFailureMessage(new Error("x"), "Failed")).toBe("Failed");
  });
});
