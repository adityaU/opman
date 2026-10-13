/**
 * The engine palette shows each runner's remaining usage, and shimmer placeholders —
 * never "Loading…" text — while usage, models or agents are still arriving.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import { EnginePalette } from "../engine-picker/EnginePalette";
import { UsageRing } from "../engine-picker/UsageRing";
import type { EngineOptions } from "../engine-picker/useEngineOptions";
import { resetUsageStore } from "../usage/usageStore";
import type { UsageAccount } from "../usage/usageClient";

vi.mock("../engine-picker/startRunner", () => ({ startRunner: vi.fn(() => Promise.resolve()) }));

const soon = (ms: number) => new Date(Date.now() + ms).toISOString();

const REPORT = {
  accounts: [
    {
      id: "claude", label: "Claude", state: "ok", plan: "Max",
      windows: [
        { id: "session", label: "5-hour", usedPercent: 38, resetsAt: soon(112 * 60_000 + 20_000), severity: "normal" },
        { id: "weekly", label: "Weekly", usedPercent: 85, resetsAt: null, severity: "warning" },
      ],
      credits: null, note: null, stale: false, fetchedAt: new Date().toISOString(),
    },
    {
      id: "codex", label: "Codex", state: "expired", plan: null, windows: [],
      credits: null, note: null, stale: true, fetchedAt: new Date().toISOString(),
    },
  ],
  runners: { claude: "claude", codex: "codex" },
};

const OPTIONS: EngineOptions = {
  models: [{ providerId: "anthropic", providerName: "Anthropic", modelId: "sonnet", modelName: "Sonnet 5.5", isDefault: true, isConnected: true }],
  agents: [{ id: "build", label: "Build", description: "Writes code" } as EngineOptions["agents"][number]],
  permissionModes: null,
  loading: false,
  loadingModels: false,
  loadingAgents: false,
};

function renderPalette(options: Partial<EngineOptions> = {}) {
  return render(
    <EnginePalette
      runner="claude"
      availableRunners={["claude", "codex", "opencode"]}
      selectedModel={{ providerID: "anthropic", modelID: "sonnet" }}
      selectedAgent="build"
      supportedEfforts={[]}
      effort={null}
      permission="default"
      onRunnerChange={vi.fn()}
      onModelSelected={vi.fn()}
      onAgentChange={vi.fn()}
      onEffortChange={vi.fn()}
      onPermissionChange={vi.fn()}
      onClose={vi.fn()}
      options={{ ...OPTIONS, ...options }}
    />,
  );
}

let fetchMock: ReturnType<typeof vi.fn>;
beforeEach(() => {
  resetUsageStore();
  fetchMock = vi.fn(() => Promise.resolve(new Response(JSON.stringify(REPORT), { status: 200 })));
  vi.stubGlobal("fetch", fetchMock);
});
afterEach(() => vi.unstubAllGlobals());

describe("EnginePalette usage", () => {
  it("shows what each runner has left, with reset times and state notes", async () => {
    renderPalette();
    const claude = await screen.findByRole("option", { name: "Claude" });
    await waitFor(() => expect(within(claude).getAllByRole("meter")).toHaveLength(2));
    const [session, weekly] = within(claude).getAllByRole("meter");
    expect(session).toHaveAttribute("aria-valuenow", "62");
    expect(session).toHaveAttribute("aria-valuetext", "62% left, resets in 1h 52m");
    expect(weekly.closest(".usage-meter")).toHaveAttribute("data-severity", "warning");
    expect(within(claude).getByText("Max")).toBeInTheDocument();

    const codex = screen.getByRole("option", { name: "Codex" });
    expect(codex).toHaveTextContent("Open codex once to refresh its login");
    expect(within(codex).getByText(/Reading from/)).toBeInTheDocument();

    const opencode = screen.getByRole("option", { name: "OpenCode" });
    expect(opencode).toHaveTextContent("Usage not reported");
    expect(fetchMock).toHaveBeenCalledWith("/api/usage", expect.anything());
  });

  it("keeps runner rows named by the runner alone", async () => {
    renderPalette();
    await screen.findAllByRole("meter");
    // "Claude" must stay an exact name: usage text goes in the description.
    expect(screen.getByRole("option", { name: "Claude" })).toHaveAccessibleDescription("5-hour: 62% left, resets in 1h 52m. Weekly: 15% left");
  });

  it("draws meter skeletons while usage loads, never loading text", () => {
    fetchMock.mockImplementation(() => new Promise(() => {}));
    const { container } = renderPalette();
    expect(container.ownerDocument.querySelectorAll(".usage-meters[data-loading] .skel").length).toBeGreaterThan(0);
    expect(screen.queryByText(/Loading…|Loading\.\.\./)).toBeNull();
  });

  it("draws model and agent row skeletons while the runner's lists load", () => {
    renderPalette({ models: [], agents: [], loadingModels: true, loadingAgents: true, loading: true });
    expect(screen.getByRole("status", { name: /Loading Claude models/ })).toBeInTheDocument();
    expect(screen.getByRole("status", { name: /Loading Claude agents/ })).toBeInTheDocument();
    expect(screen.queryByText(/Nothing matches/)).toBeNull();
  });

  it("says once that usage is unavailable on a server without the route", async () => {
    fetchMock.mockImplementation(() => Promise.resolve(new Response("", { status: 404 })));
    renderPalette();
    expect(await screen.findByText("Usage not available")).toBeInTheDocument();
    expect(screen.queryAllByRole("meter")).toHaveLength(0);
    expect(document.querySelectorAll(".skel")).toHaveLength(0);
  });

  it("refresh forces a re-read", async () => {
    renderPalette();
    await screen.findAllByRole("meter");
    screen.getByRole("button", { name: "Refresh usage" }).click();
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/usage?refresh=1", expect.anything()));
  });
});

describe("UsageRing", () => {
  const base = REPORT.accounts[0] as unknown as UsageAccount;

  it("labels the tightest window for assistive tech", () => {
    render(<UsageRing account={base} runnerLabel="Claude" />);
    const ring = screen.getByRole("img");
    expect(ring.getAttribute("aria-label")).toMatch(/^Claude usage: 5-hour: 62% left/);
    expect(ring).toHaveAttribute("data-severity", "warning");
  });

  it("draws nothing without an account or windows", () => {
    const { container } = render(<UsageRing account={null} runnerLabel="Claude" />);
    expect(container).toBeEmptyDOMElement();
    const { container: empty } = render(
      <UsageRing account={{ ...base, state: "api_key", windows: [] }} runnerLabel="Claude" />,
    );
    expect(empty).toBeEmptyDOMElement();
  });

  it("swaps the gauge for a warning when the login has expired", () => {
    render(<UsageRing account={{ ...base, state: "expired" }} runnerLabel="Claude" />);
    expect(screen.getByRole("img").className).toContain("is-alert");
  });
});
