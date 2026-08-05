import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { StartupGate } from "../StartupGate";
import type { AppState } from "../api";

const state = (startup_ready: boolean): AppState => ({
  startup_ready,
  projects: [],
  active_project: 0,
  panels: { sidebar: true, terminal_pane: false, neovim_pane: false, integrated_terminal: false, git_panel: false },
  focused: "ChatInput",
});

describe("StartupGate", () => {
  it("shows session hydration as the active step while the backend is starting", () => {
    render(
      <StartupGate
        appState={state(false)}
        connectionStatus="reconnecting"
        initialConnectionsReady={false}
        activeSessionId={null}
        isLoadingMessages={false}
        providersLoading={false}
      />,
    );

    expect(screen.getByText("Hydrate sessions")).toBeTruthy();
    expect(screen.getByText("Loading")).toBeTruthy();
    expect(screen.queryByText("Preparing your workspace")).toBeTruthy();
  });

  it("marks all steps complete only after live updates and tools are ready", () => {
    render(
      <StartupGate
        appState={state(true)}
        connectionStatus="connected"
        initialConnectionsReady={true}
        activeSessionId="session-1"
        isLoadingMessages={false}
        providersLoading={false}
      />,
    );

    expect(screen.queryByText("Loading")).toBeNull();
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "5");
  });
});
