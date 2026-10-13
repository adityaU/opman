/**
 * The usage client and its shared store: every failure mode of `/api/usage` must land as
 * something the picker can render, and reopening the palette within a minute must not
 * cost another request.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { fetchUsage, parseUsageReport } from "../usage/usageClient";
import { getUsageSnapshot, loadUsage, resetUsageStore, USAGE_FRESH_MS } from "../usage/usageStore";
import { useRunnerUsage, useUsage } from "../usage/useUsage";

const REPORT = {
  accounts: [{
    id: "claude",
    label: "Claude",
    state: "ok",
    plan: "Max",
    windows: [
      { id: "session", label: "5-hour", usedPercent: 38, resetsAt: "2026-10-12T12:00:00Z", severity: "normal" },
      { id: "weekly", label: "Weekly", usedPercent: 12, resetsAt: null, severity: null },
    ],
    credits: null,
    note: null,
    stale: false,
    fetchedAt: "2026-10-12T10:00:00Z",
  }],
  runners: { claude: "claude", "claude-code": "claude" },
};

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  resetUsageStore();
  fetchMock = vi.fn();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("fetchUsage", () => {
  it("reads a report from /api/usage", async () => {
    fetchMock.mockResolvedValue(json(REPORT));
    const result = await fetchUsage();
    expect(fetchMock.mock.calls[0][0]).toBe("/api/usage");
    expect(result.kind).toBe("ok");
  });

  it("asks the server to re-read with ?refresh=1", async () => {
    fetchMock.mockResolvedValue(json(REPORT));
    await fetchUsage({ refresh: true });
    expect(fetchMock.mock.calls[0][0]).toBe("/api/usage?refresh=1");
  });

  it("treats a 404 as a server without usage, not a failure", async () => {
    fetchMock.mockResolvedValue(new Response("Not Found", { status: 404 }));
    expect(await fetchUsage()).toEqual({ kind: "unavailable" });
  });

  it("treats the SPA's index.html fallback as a server without usage", async () => {
    fetchMock.mockResolvedValue(new Response("<!doctype html><html></html>", { status: 200 }));
    expect(await fetchUsage()).toEqual({ kind: "unavailable" });
  });

  it("surfaces the server's own message on a 500", async () => {
    fetchMock.mockResolvedValue(json({ error: "keychain locked" }, 500));
    expect(await fetchUsage()).toEqual({ kind: "error", message: "keychain locked" });
  });

  it("turns a network failure into an error result instead of throwing", async () => {
    fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));
    expect(await fetchUsage()).toEqual({ kind: "error", message: "Failed to fetch" });
  });
});

describe("parseUsageReport", () => {
  it("drops malformed windows and accounts, clamps percentages, tolerates unknown states", () => {
    const report = parseUsageReport({
      accounts: [
        { id: "codex", state: "brand_new_state", windows: [{ id: "w", usedPercent: 140 }, { id: "bad" }, "junk"] },
        { label: "no id" },
      ],
      runners: { codex: "codex", broken: 7 },
    });
    expect(report?.accounts).toHaveLength(1);
    expect(report?.accounts[0].state).toBe("unsupported");
    expect(report?.accounts[0].windows).toEqual([
      { id: "w", label: "w", usedPercent: 100, resetsAt: null, severity: null },
    ]);
    expect(report?.runners).toEqual({ codex: "codex" });
  });

  it("rejects a body that is not a report", () => {
    expect(parseUsageReport({ hello: 1 })).toBeNull();
    expect(parseUsageReport(null)).toBeNull();
  });
});

describe("usage store", () => {
  it("serves a fresh report from memory and shares one in-flight request", async () => {
    fetchMock.mockResolvedValue(json(REPORT));
    await Promise.all([loadUsage(), loadUsage(), loadUsage()]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    await loadUsage();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(getUsageSnapshot().status).toBe("ready");
  });

  it("refetches once the freshness window has passed", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    fetchMock.mockResolvedValue(json(REPORT));
    await loadUsage();
    vi.setSystemTime(Date.now() + USAGE_FRESH_MS + 1);
    await loadUsage();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("force bypasses freshness and asks the server to refresh", async () => {
    fetchMock.mockResolvedValue(json(REPORT));
    await loadUsage();
    await loadUsage({ force: true });
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[1][0]).toBe("/api/usage?refresh=1");
  });

  it("keeps the last good report through a failed refresh", async () => {
    fetchMock.mockResolvedValueOnce(json(REPORT)).mockResolvedValueOnce(json({ error: "boom" }, 502));
    await loadUsage();
    await loadUsage({ force: true });
    const snap = getUsageSnapshot();
    expect(snap.report?.accounts[0].plan).toBe("Max");
    expect(snap.status).toBe("ready");
    expect(snap.error).toBe("boom");
  });

  it("remembers an unavailable server for the freshness window", async () => {
    fetchMock.mockResolvedValue(new Response("", { status: 404 }));
    await loadUsage();
    await loadUsage();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(getUsageSnapshot().status).toBe("unavailable");
  });

  it("retries an error on the next load rather than caching it", async () => {
    fetchMock.mockResolvedValueOnce(json({ error: "boom" }, 500)).mockResolvedValueOnce(json(REPORT));
    await loadUsage();
    expect(getUsageSnapshot().status).toBe("error");
    await loadUsage();
    expect(getUsageSnapshot().status).toBe("ready");
  });
});

describe("useUsage / useRunnerUsage", () => {
  it("loads on mount and resolves a runner to its account", async () => {
    fetchMock.mockResolvedValue(json(REPORT));
    const { result } = renderHook(() => useRunnerUsage("claude-code"));
    expect(result.current.usage.status).toBe("loading");
    await waitFor(() => expect(result.current.account?.id).toBe("claude"));
  });

  it("returns no account for a runner the report does not map", async () => {
    fetchMock.mockResolvedValue(json(REPORT));
    const { result } = renderHook(() => useRunnerUsage("opencode"));
    await waitFor(() => expect(result.current.usage.status).toBe("ready"));
    expect(result.current.account).toBeNull();
  });

  it("observes without fetching when load is off, and refresh forces a read", async () => {
    fetchMock.mockResolvedValue(json(REPORT));
    const { result } = renderHook(() => useUsage({ load: false }));
    expect(fetchMock).not.toHaveBeenCalled();
    await act(() => result.current.refresh());
    expect(fetchMock.mock.calls[0][0]).toBe("/api/usage?refresh=1");
    expect(result.current.report?.accounts).toHaveLength(1);
  });
});
