import { describe, it, expect } from "vitest";
import {
  accountForRunner, describeUsage, formatAge, formatDuration, formatResetIn, mostConstrained,
  noteTone, remainingPercent, severityOf, stateNote,
} from "../usage/format";
import type { UsageAccount, UsageWindow } from "../usage/usageClient";

const NOW = Date.parse("2026-10-12T10:00:00Z");
const win = (over: Partial<UsageWindow> = {}): UsageWindow => ({
  id: "session", label: "5-hour", usedPercent: 38, resetsAt: null, severity: null, ...over,
});
const account = (over: Partial<UsageAccount> = {}): UsageAccount => ({
  id: "claude", label: "Claude", state: "ok", plan: "Max", windows: [win()],
  credits: null, note: null, stale: false, fetchedAt: null, ...over,
});

describe("remainingPercent", () => {
  it("is 100 minus used, rounded and clamped", () => {
    expect(remainingPercent({ usedPercent: 38 })).toBe(62);
    expect(remainingPercent({ usedPercent: 37.6 })).toBe(62);
    expect(remainingPercent({ usedPercent: 120 })).toBe(0);
    expect(remainingPercent({ usedPercent: -5 })).toBe(100);
    expect(remainingPercent({ usedPercent: Number.NaN })).toBe(0);
  });
});

describe("severityOf", () => {
  it("prefers the server's severity", () => {
    expect(severityOf(win({ usedPercent: 99, severity: "normal" }))).toBe("normal");
  });
  it("derives one from what is left otherwise", () => {
    expect(severityOf(win({ usedPercent: 50 }))).toBe("normal");
    expect(severityOf(win({ usedPercent: 80 }))).toBe("warning");
    expect(severityOf(win({ usedPercent: 95 }))).toBe("critical");
  });
});

describe("reset and age formatting", () => {
  it("formats durations compactly", () => {
    expect(formatDuration(30_000)).toBe("<1m");
    expect(formatDuration(42 * 60_000)).toBe("42m");
    expect(formatDuration((60 + 52) * 60_000)).toBe("1h 52m");
    expect(formatDuration(3 * 3_600_000)).toBe("3h");
    expect(formatDuration((3 * 24 + 4) * 3_600_000)).toBe("3d 4h");
    expect(formatDuration(2 * 24 * 3_600_000)).toBe("2d");
  });

  it("says when a window resets, relative to now", () => {
    expect(formatResetIn("2026-10-12T11:52:00Z", NOW)).toBe("resets in 1h 52m");
    expect(formatResetIn("2026-10-12T09:00:00Z", NOW)).toBe("resets now");
    expect(formatResetIn(null, NOW)).toBeNull();
    expect(formatResetIn("not a date", NOW)).toBeNull();
  });

  it("says how old a reading is", () => {
    expect(formatAge("2026-10-12T09:59:30Z", NOW)).toBe("just now");
    expect(formatAge("2026-10-12T09:46:00Z", NOW)).toBe("14m ago");
    expect(formatAge(null, NOW)).toBeNull();
  });
});

describe("account helpers", () => {
  it("picks the window with the least left", () => {
    const a = account({ windows: [win({ id: "a", usedPercent: 20 }), win({ id: "b", usedPercent: 90 })] });
    expect(mostConstrained(a)?.id).toBe("b");
    expect(mostConstrained(account({ windows: [] }))).toBeNull();
    expect(mostConstrained(null)).toBeNull();
  });

  it("maps a runner to its account, null when unmapped", () => {
    const report = { accounts: [account()], runners: { claude: "claude", ghost: "nobody" } };
    expect(accountForRunner(report, "claude")?.id).toBe("claude");
    expect(accountForRunner(report, "opencode")).toBeNull();
    expect(accountForRunner(report, "ghost")).toBeNull();
    expect(accountForRunner(null, "claude")).toBeNull();
  });

  it("writes a note per state, preferring the server's", () => {
    expect(stateNote(account())).toBeNull();
    expect(stateNote(account({ state: "expired" }))).toBe("Open `claude` once to refresh its login");
    expect(stateNote(account({ state: "api_key" }))).toBe("API key — pay as you go");
    expect(stateNote(account({ state: "unsupported" }))).toBe("Usage not reported");
    expect(stateNote(account({ state: "error", note: "Keychain locked" }))).toBe("Keychain locked");
    expect(noteTone(account({ state: "expired" }))).toBe("warning");
    expect(noteTone(account({ state: "api_key" }))).toBe("muted");
  });

  it("describes an account in one sentence for assistive tech", () => {
    const a = account({ stale: true, windows: [win({ resetsAt: "2026-10-12T11:52:00Z" })] });
    expect(describeUsage(a, NOW)).toBe("5-hour: 62% left, resets in 1h 52m. last reading may be out of date");
    expect(describeUsage(null, NOW)).toBe("Usage not reported");
    expect(describeUsage(account({ state: "expired", windows: [] }), NOW)).toBe("Open claude once to refresh its login");
  });
});

describe("planLabel", () => {
  it("turns plan ids into tags", async () => {
    const { planLabel } = await import("../usage/format");
    expect(planLabel("pro")).toBe("Pro");
    expect(planLabel("plus")).toBe("Plus");
    expect(planLabel("max_5x")).toBe("Max 5x");
  });
});
