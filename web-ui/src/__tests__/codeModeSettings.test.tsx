import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ServersSection } from "../settings-page/mcp/ServersSection";
import type { McpServer, McpServerDraft } from "../api/mcp";

const api = vi.hoisted(() => ({ list: vi.fn(), save: vi.fn(), tools: vi.fn() }));
vi.mock("../api/mcp", async () => ({
  ...await vi.importActual("../api/mcp"),
  fetchMcpServers: api.list,
  saveMcpServer: api.save,
  fetchMcpServerTools: api.tools,
}));

let servers: McpServer[];
const server = (name: string, extra: Partial<McpServer> = {}): McpServer => ({
  name, transport: "stdio", auth: "none", enabled: true, builtin: true,
  args: [], envNames: [], headerNames: [], runners: [], proxied: false,
  needsOpencodeRestart: false, authenticated: false, codeMode: true,
  codeModeDisabledTools: [], ...extra,
});

beforeEach(() => {
  servers = [server("code"), server("time", { enabled: false, codeModeDisabledTools: ["old_tool"] })];
  api.list.mockImplementation(async () => servers);
  api.save.mockImplementation(async (name: string, draft: McpServerDraft) => {
    servers = servers.map(entry => entry.name === name ? {
      ...entry,
      codeMode: draft.codeMode ?? entry.codeMode,
      codeModeDisabledTools: draft.codeModeDisabledTools ?? entry.codeModeDisabledTools,
    } : entry);
  });
  api.tools.mockResolvedValue({ status: "listed", tools: [
    { name: "now", description: "Read the current time.", inputSchema: { type: "object" } },
  ] });
});

afterEach(() => { cleanup(); vi.resetAllMocks(); });

describe("code mode settings", () => {
  it("changes code mode access independently from direct server access", async () => {
    render(<ServersSection onError={vi.fn()} />);
    const toggle = await screen.findByRole("switch", { name: "Use time in code mode" });
    expect(toggle).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("switch", { name: "Enable time" })).toHaveAttribute("aria-checked", "false");
    expect(screen.queryByRole("switch", { name: "Use code in code mode" })).toBeNull();

    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "false"));
    expect(api.save).toHaveBeenCalledWith("time", { codeMode: false });
    expect(servers[1].enabled).toBe(false);
    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
    expect(api.save).toHaveBeenLastCalledWith("time", { codeMode: true });
  });

  it("loads tools on demand and preserves other exclusions when toggling one tool", async () => {
    render(<ServersSection onError={vi.fn()} />);
    await screen.findByRole("switch", { name: "Use time in code mode" });
    expect(api.tools).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Show code mode tools for time" }));
    const toggle = await screen.findByRole("switch", { name: "Use time.now in code mode" });
    expect(api.tools).toHaveBeenCalledWith("time");
    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "false"));
    expect(api.save).toHaveBeenCalledWith("time", { codeModeDisabledTools: ["old_tool", "now"] });
    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
    expect(api.save).toHaveBeenLastCalledWith("time", { codeModeDisabledTools: ["old_tool"] });
  });

  it("reports failed saves and keeps the existing setting", async () => {
    api.save.mockRejectedValue(new Error("Could not save MCP settings"));
    const onError = vi.fn();
    render(<ServersSection onError={onError} />);
    const toggle = await screen.findByRole("switch", { name: "Use time in code mode" });
    fireEvent.click(toggle);
    await waitFor(() => expect(onError).toHaveBeenCalledWith("Could not save MCP settings"));
    expect(toggle).toHaveAttribute("aria-checked", "true");
  });

  it("keeps the focused tool switch visible while settings events refresh its catalog", async () => {
    render(<ServersSection onError={vi.fn()} />);
    fireEvent.click(await screen.findByRole("button", { name: "Show code mode tools for time" }));
    const toggle = await screen.findByRole("switch", { name: "Use time.now in code mode" });
    toggle.focus();
    api.tools.mockImplementationOnce(() => new Promise(() => {}));
    fireEvent(window, new Event("opman:mcp-servers-changed"));
    await waitFor(() => expect(api.tools).toHaveBeenCalledTimes(2));
    expect(screen.getByRole("switch", { name: "Use time.now in code mode" })).toBe(toggle);
    expect(document.activeElement).toBe(toggle);
  });
});
