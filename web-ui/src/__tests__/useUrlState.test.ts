/**
 * Unit tests for useUrlState utilities (readUrlState, buildSearchString).
 * The hook itself is tightly coupled with browser history APIs, so we test
 * the pure parsing/serialization helpers and basic hook behavior.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { readUrlState, useUrlState } from "../hooks/useUrlState";

// ── Mock window.location.search ─────────────────────────
let mockSearch = "";
Object.defineProperty(window, "location", {
  value: {
    search: "",
    pathname: "/",
    reload: vi.fn(),
  },
  writable: true,
});

// ── Mock history API ─────────────────────────────────────
const pushStateSpy = vi.fn();
const replaceStateSpy = vi.fn();
Object.defineProperty(window, "history", {
  value: {
    pushState: pushStateSpy,
    replaceState: replaceStateSpy,
  },
  writable: true,
});

beforeEach(() => {
  window.location.search = "";
  window.location.pathname = "/";
  pushStateSpy.mockClear();
  replaceStateSpy.mockClear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ═══════════════════════════════════════════════════════
// readUrlState
// ═══════════════════════════════════════════════════════

describe("readUrlState", () => {
  it("returns defaults when no params present", () => {
    window.location.search = "";
    const state = readUrlState();
    expect(state.sessionId).toBeNull();
    expect(state.projectIdx).toBeNull();
    expect(state.panels.sidebar).toBe(true); // default open
    expect(state.panels.terminal).toBe(false);
    expect(state.panels.editor).toBe(false);
    expect(state.panels.git).toBe(false);
  });

  it("parses session and project from URL", () => {
    window.location.search = "?session=abc123&project=2";
    const state = readUrlState();
    expect(state.sessionId).toBe("abc123");
    expect(state.projectIdx).toBe(2);
  });

  it("parses panel booleans", () => {
    window.location.search = "?sidebar=0&terminal=1&editor=true&git=false";
    const state = readUrlState();
    expect(state.panels.sidebar).toBe(false);
    expect(state.panels.terminal).toBe(true);
    expect(state.panels.editor).toBe(true);
    expect(state.panels.git).toBe(false);
  });
});

// ═══════════════════════════════════════════════════════
// useUrlState hook
// ═══════════════════════════════════════════════════════

describe("useUrlState", () => {
  it("calls replaceState on mount", () => {
    renderHook(() =>
      useUrlState({
        sessionId: "sess1",
        projectIdx: 0,
        panels: { sidebar: true, terminal: false, editor: false, git: false },
        onPopState: vi.fn(),
      })
    );
    // replaceState should have been called (initial mount + state sync)
    expect(replaceStateSpy).toHaveBeenCalled();
  });

  it("uses pushState when session changes", () => {
    const { rerender } = renderHook(
      ({ sessionId }) =>
        useUrlState({
          sessionId,
          projectIdx: 0,
          panels: { sidebar: true, terminal: false, editor: false, git: false },
          onPopState: vi.fn(),
        }),
      { initialProps: { sessionId: "sess1" } }
    );

    pushStateSpy.mockClear();
    replaceStateSpy.mockClear();

    rerender({ sessionId: "sess2" });
    expect(pushStateSpy).toHaveBeenCalled();
  });

  it("uses replaceState for panel toggle (same session)", () => {
    const panels1 = { sidebar: true, terminal: false, editor: false, git: false };
    const panels2 = { sidebar: true, terminal: true, editor: false, git: false };

    const { rerender } = renderHook(
      ({ panels }) =>
        useUrlState({
          sessionId: "sess1",
          projectIdx: 0,
          panels,
          onPopState: vi.fn(),
        }),
      { initialProps: { panels: panels1 } }
    );

    pushStateSpy.mockClear();
    replaceStateSpy.mockClear();

    rerender({ panels: panels2 });
    expect(replaceStateSpy).toHaveBeenCalled();
    expect(pushStateSpy).not.toHaveBeenCalled();
  });
});
