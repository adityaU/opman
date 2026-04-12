/**
 * Unit tests for SessionSearchModal pure functions (fuzzyMatch, formatTimestamp)
 * and component behavior.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { fuzzyMatch, formatTimestamp } from "../SessionSearchModal";

// ── fuzzyMatch ──────────────────────────────────────────
describe("fuzzyMatch", () => {
  it("empty query matches everything with score 0", () => {
    const result = fuzzyMatch("", "anything");
    expect(result).not.toBeNull();
    expect(result!.score).toBe(0);
    expect(result!.indices).toEqual([]);
  });

  it("exact substring at start gives score 0", () => {
    const result = fuzzyMatch("abc", "abcdef");
    expect(result).not.toBeNull();
    expect(result!.score).toBe(0);
    expect(result!.indices).toEqual([0, 1, 2]);
  });

  it("substring not at start gives score equal to start position", () => {
    const result = fuzzyMatch("def", "abcdef");
    expect(result).not.toBeNull();
    expect(result!.score).toBe(3); // starts at index 3
    expect(result!.indices).toEqual([3, 4, 5]);
  });

  it("is case-insensitive", () => {
    const result = fuzzyMatch("ABC", "xyzabcdef");
    expect(result).not.toBeNull();
    expect(result!.indices).toEqual([3, 4, 5]);
  });

  it("returns null when no match is possible", () => {
    expect(fuzzyMatch("xyz", "abc")).toBeNull();
  });

  it("fuzzy matches characters in order with gaps", () => {
    const result = fuzzyMatch("ac", "abcde");
    // "a" at 0, "c" at 2 — substring "ac" not found, so fuzzy match
    // Actually "ac" is not a substring in "abcde" so it goes fuzzy
    // a(0), c(2) => gaps = 2 - 0 - 1 = 1
    expect(result).not.toBeNull();
    expect(result!.indices).toEqual([0, 2]);
    // score = 100 + gaps*10 + indices[0] + (target.len - query.len)
    // = 100 + 1*10 + 0 + (5 - 2) = 113
    expect(result!.score).toBe(113);
  });

  it("returns null when not all query chars can be matched", () => {
    expect(fuzzyMatch("zxy", "axbycz")).toBeNull();
  });

  it("prefers substring match over fuzzy match (lower score)", () => {
    const substring = fuzzyMatch("ab", "xab");
    const fuzzy = fuzzyMatch("ab", "a_b");
    expect(substring).not.toBeNull();
    expect(fuzzy).not.toBeNull();
    // Substring score should be lower (better) than fuzzy score
    expect(substring!.score).toBeLessThan(fuzzy!.score);
  });
});

// ── formatTimestamp ─────────────────────────────────────
describe("formatTimestamp", () => {
  let realDateNow: typeof Date.now;

  beforeEach(() => {
    realDateNow = Date.now;
  });

  afterEach(() => {
    Date.now = realDateNow;
    vi.useRealTimers();
  });

  it("returns empty string for falsy timestamp", () => {
    expect(formatTimestamp(0)).toBe("");
  });

  it('returns "just now" for < 60 seconds ago', () => {
    const nowMs = Date.now();
    // ts is in seconds
    const ts = Math.floor(nowMs / 1000) - 30; // 30 seconds ago
    // We need to freeze time so "now" is stable
    vi.useFakeTimers();
    vi.setSystemTime(nowMs);

    expect(formatTimestamp(ts)).toBe("just now");
  });

  it('returns "Xm ago" for < 1 hour', () => {
    const nowMs = Date.now();
    const ts = Math.floor(nowMs / 1000) - 5 * 60; // 5 minutes ago
    vi.useFakeTimers();
    vi.setSystemTime(nowMs);

    expect(formatTimestamp(ts)).toBe("5m ago");
  });

  it('returns "Xh ago" for < 24 hours', () => {
    const nowMs = Date.now();
    const ts = Math.floor(nowMs / 1000) - 3 * 3600; // 3 hours ago
    vi.useFakeTimers();
    vi.setSystemTime(nowMs);

    expect(formatTimestamp(ts)).toBe("3h ago");
  });

  it('returns "Xd ago" for < 7 days', () => {
    const nowMs = Date.now();
    const ts = Math.floor(nowMs / 1000) - 2 * 86400; // 2 days ago
    vi.useFakeTimers();
    vi.setSystemTime(nowMs);

    expect(formatTimestamp(ts)).toBe("2d ago");
  });

  it("returns formatted date for >= 7 days", () => {
    const nowMs = Date.now();
    const ts = Math.floor(nowMs / 1000) - 30 * 86400; // 30 days ago
    vi.useFakeTimers();
    vi.setSystemTime(nowMs);

    const result = formatTimestamp(ts);
    // Should be something like "Feb 6" or locale-dependent
    expect(result).not.toBe("");
    expect(result).not.toContain("ago");
  });
});
