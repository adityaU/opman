/**
 * Settings: each server picks its own theme (saved on that server), and the projects
 * list can be added to and removed from.
 */
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

const fetchServerTheme = vi.fn();
const setServerTheme = vi.fn();
vi.mock("../api/serverTheme", () => ({
  fetchServerTheme: (...args: unknown[]) => fetchServerTheme(...args),
  setServerTheme: (...args: unknown[]) => setServerTheme(...args),
}));

const fetchAppState = vi.fn();
const removeProject = vi.fn();
vi.mock("../api", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("../api");
  return {
    ...actual,
    fetchAppState: (...args: unknown[]) => fetchAppState(...args),
    removeProject: (...args: unknown[]) => removeProject(...args),
  };
});

import { ServerThemePicker } from "../settings-page/servers/ServerThemePicker";
import { ProjectsSection } from "../settings-page/projects/ProjectsSection";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const colors = {} as never;

describe("ServerThemePicker", () => {
  it("shows the server's own theme and saves a change on that server", async () => {
    fetchServerTheme.mockResolvedValue({ name: "nord", dark: colors, light: colors });
    setServerTheme.mockResolvedValue({ name: "dracula", dark: colors, light: colors });
    render(<ServerThemePicker serverId="lab" serverName="Lab box" themes={["dracula", "nord"]} />);
    const select = (await screen.findByLabelText("Theme for Lab box")) as HTMLSelectElement;
    await waitFor(() => expect(select.value).toBe("nord"));
    expect(fetchServerTheme).toHaveBeenCalledWith("lab");
    fireEvent.change(select, { target: { value: "dracula" } });
    expect(setServerTheme).toHaveBeenCalledWith("lab", "dracula");
  });

  it("goes back to the old theme when saving fails", async () => {
    fetchServerTheme.mockResolvedValue({ name: "nord", dark: colors, light: colors });
    setServerTheme.mockRejectedValue(new Error("down"));
    render(<ServerThemePicker serverId="lab" serverName="Lab" themes={["dracula", "nord"]} />);
    const select = (await screen.findByLabelText("Theme for Lab")) as HTMLSelectElement;
    await waitFor(() => expect(select.value).toBe("nord"));
    fireEvent.change(select, { target: { value: "dracula" } });
    await waitFor(() => expect(select.value).toBe("nord"));
  });

  it("hides itself for a server it cannot read", async () => {
    fetchServerTheme.mockRejectedValue(new Error("unreachable"));
    const { container } = render(<ServerThemePicker serverId="x" serverName="X" themes={["nord"]} />);
    await waitFor(() => expect(container.textContent).toBe(""));
  });
});

describe("ProjectsSection", () => {
  const project = (index: number, name: string) => ({
    name, path: `/work/${name}`, index, active_session: null, sessions: [], session_count: 3,
    git_branch: "main", busy_sessions: [],
  });

  it("lists projects and removes one after confirming", async () => {
    fetchAppState
      .mockResolvedValueOnce({ projects: [project(0, "alpha"), project(1, "beta")] })
      .mockResolvedValueOnce({ projects: [project(0, "alpha")] });
    removeProject.mockResolvedValue(undefined);
    render(<ProjectsSection onError={vi.fn()} />);
    expect(await screen.findByText("beta")).toBeTruthy();
    expect(screen.getByText("/work/beta")).toBeTruthy();
    fireEvent.click(screen.getByLabelText("Remove beta"));
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(removeProject).toHaveBeenCalledWith(1));
    await waitFor(() => expect(screen.queryByText("beta")).toBeNull());
  });

  it("keeps the project when the removal is not confirmed", async () => {
    fetchAppState.mockResolvedValue({ projects: [project(0, "alpha")] });
    render(<ProjectsSection onError={vi.fn()} />);
    fireEvent.click(await screen.findByLabelText("Remove alpha"));
    fireEvent.click(screen.getByRole("button", { name: "Keep" }));
    expect(removeProject).not.toHaveBeenCalled();
    expect(screen.getByText("alpha")).toBeTruthy();
  });
});
