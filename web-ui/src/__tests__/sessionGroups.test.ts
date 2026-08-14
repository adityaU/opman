import { describe, it, expect } from "vitest";
import { bucketFor, groupSessions } from "../sidebar/sessionGroups";
import type { SessionInfo } from "../api";

/** 14 March 2025, 10:00 local. */
const NOW = new Date(2025, 2, 14, 10, 0, 0).getTime();
const HOUR = 3_600_000;
const DAY = 86_400_000;

function at(id: string, updated: number): SessionInfo {
  return {
    id,
    title: id,
    parentID: "",
    directory: "/p",
    time: { created: updated, updated },
  };
}

describe("bucketFor", () => {
  it("puts anything since local midnight in today", () => {
    expect(bucketFor(NOW, NOW)).toBe("today");
    expect(bucketFor(new Date(2025, 2, 14, 0, 0, 0).getTime(), NOW)).toBe("today");
  });

  it("counts days from local midnight, not from a rolling 24 hours", () => {
    // 11pm last night is 11 hours ago — yesterday, not today.
    const lastNight = new Date(2025, 2, 13, 23, 0, 0).getTime();
    expect(bucketFor(lastNight, NOW)).toBe("yesterday");
  });

  it("separates the last week from everything older", () => {
    expect(bucketFor(NOW - 3 * DAY, NOW)).toBe("week");
    expect(bucketFor(NOW - 6 * DAY, NOW)).toBe("week");
    expect(bucketFor(NOW - 30 * DAY, NOW)).toBe("older");
  });

  it("treats a clock-skewed future timestamp as today rather than dropping it", () => {
    expect(bucketFor(NOW + HOUR, NOW)).toBe("today");
  });
});

describe("groupSessions", () => {
  it("emits buckets newest-first and skips the empty ones", () => {
    const { groups } = groupSessions(
      [at("a", NOW), at("b", NOW - 30 * DAY)],
      NOW,
    );

    expect(groups.map((g) => g.bucket)).toEqual(["today", "older"]);
    expect(groups.map((g) => g.label)).toEqual(["Today", "Older"]);
  });

  it("keeps the order it was given inside each bucket", () => {
    const { groups } = groupSessions(
      [at("newer", NOW), at("older", NOW - HOUR)],
      NOW,
    );

    expect(groups[0].sessions.map((s) => s.id)).toEqual(["newer", "older"]);
  });

  it("holds pinned sessions above the headings instead of dating them", () => {
    const { pinned, groups } = groupSessions(
      [at("a", NOW), at("pin", NOW - 30 * DAY)],
      NOW,
      new Set(["pin"]),
    );

    expect(pinned.map((s) => s.id)).toEqual(["pin"]);
    expect(groups.map((g) => g.bucket)).toEqual(["today"]);
  });

  it("returns nothing to render for an empty list", () => {
    const { pinned, groups } = groupSessions([], NOW);

    expect(pinned).toEqual([]);
    expect(groups).toEqual([]);
  });
});
