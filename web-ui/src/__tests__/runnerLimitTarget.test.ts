/**
 * Where a conversation goes when its runner runs low: the user's own list first, in their
 * order, then suggestions from the usage report — and never somewhere that would fail the
 * same way.
 */
import { describe, expect, it } from "vitest";
import { limitOptions, type TargetInput } from "../runner-limit/chooseTarget";
import { levelOfWindow } from "../runner-limit/limitState";
import { LEGACY_DEFAULTS, parseFallbacks } from "../runner-limit/legacyFallbacks";
import type { UsageAccount, UsageReport } from "../usage/usageClient";

const NOW = Date.parse("2026-10-14T12:00:00Z");
const LATER = "2026-10-14T15:00:00Z";

function account(id: string, usedPercent: number): UsageAccount {
  return {
    id, label: id, state: "ok", plan: null, credits: null, note: null, stale: false, fetchedAt: null,
    windows: [{ id: "5h", label: "5-hour", usedPercent, resetsAt: LATER, severity: null }],
  };
}

const REPORT: UsageReport = {
  accounts: [account("anthropic", 96), account("openai", 40)],
  // Claude and Claude Code share one account: moving between them gets nowhere.
  runners: { claude: "anthropic", "claude-code": "anthropic", codex: "openai" },
};

const RUNNERS = ["claude", "claude-code", "codex", "opencode"];

function input(overrides: Partial<TargetInput> = {}): TargetInput {
  return {
    report: REPORT, current: "claude", level: "near", pairs: [], available: RUNNERS,
    now: NOW, nearPercent: 5, ...overrides,
  };
}

describe("levelOfWindow", () => {
  const window = (usedPercent: number) => ({ id: "w", label: "w", usedPercent, resetsAt: null, severity: "critical" as const });

  it("goes by the user's threshold, not the server's severity", () => {
    // The server calls a tenth left critical; a 5% threshold must not fire at 8%.
    expect(levelOfWindow(window(92), 5)).toBe("ok");
    expect(levelOfWindow(window(95), 5)).toBe("near");
    expect(levelOfWindow(window(92), 10)).toBe("near");
    expect(levelOfWindow(window(100), 5)).toBe("out");
  });
});

describe("limitOptions", () => {
  it("offers nothing while the runner is fine", () => {
    expect(limitOptions(input({ level: "ok" }))).toEqual([]);
  });

  it("suggests the runners with room, most left first, skipping the same account", () => {
    const options = limitOptions(input());
    expect(options.map((o) => o.runner)).toEqual(["codex", "opencode"]);
    expect(options[0]).toMatchObject({ source: "suggested", model: null, leftPercent: 60 });
    // opencode reports no usage: offered, but after the runner known to have room.
    expect(options[1].leftPercent).toBeNull();
  });

  it("puts the user's list first, in their order, with their model", () => {
    const model = { providerID: "openai", modelID: "gpt-5.6-sol" };
    const options = limitOptions(input({
      pairs: [{ runner: "opencode", model: null }, { runner: "codex", model, permission: "read-only" }],
    }));
    expect(options.map((o) => [o.runner, o.source])).toEqual([
      ["opencode", "preselected"],
      ["codex", "preselected"],
    ]);
    expect(options[1].model).toEqual(model);
    expect(options[1].permission).toBe("read-only");
  });

  it("skips a listed runner that would fail the same way or is not served", () => {
    const options = limitOptions(input({
      pairs: [{ runner: "claude-code", model: null }, { runner: "gone", model: null }],
    }));
    expect(options.every((o) => o.source === "suggested")).toBe(true);
  });

  it("drops a runner that is out itself", () => {
    const report: UsageReport = { ...REPORT, accounts: [account("anthropic", 96), account("openai", 100)] };
    expect(limitOptions(input({ report })).map((o) => o.runner)).toEqual(["opencode"]);
  });
});

describe("parseFallbacks (the old per-browser record, moved to the server once)", () => {
  it("reads a stored list and threshold", () => {
    const raw = JSON.stringify({
      nearPercent: 10,
      pairs: [{ runner: "codex", model: { providerID: "openai", modelID: "gpt" } }, { runner: "opencode", model: null }],
    });
    expect(parseFallbacks(raw)).toEqual({
      nearPercent: 10,
      pairs: [
        { runner: "codex", model: { providerID: "openai", modelID: "gpt" } },
        { runner: "opencode", model: null },
      ],
    });
  });

  it("survives junk without throwing", () => {
    expect(parseFallbacks(null)).toEqual(LEGACY_DEFAULTS);
    expect(parseFallbacks("{not json")).toEqual(LEGACY_DEFAULTS);
    expect(parseFallbacks(JSON.stringify({ nearPercent: 900, pairs: [{}, { runner: "codex" }] })))
      .toEqual({ nearPercent: 5, pairs: [{ runner: "codex", model: null }] });
  });
});
