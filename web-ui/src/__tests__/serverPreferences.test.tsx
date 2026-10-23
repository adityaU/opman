import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { useServerPreferences } from "../hooks/useServerPreferences";
import { parsePreferences, publishPreferences } from "../api/preferences";

const fetch = vi.hoisted(() => vi.fn());
vi.mock("../api/preferences", async () => ({
  ...await vi.importActual<Record<string, unknown>>("../api/preferences"),
  fetchPreferences: fetch,
}));
vi.mock("../utils/appearance", () => ({ setAppearance: vi.fn() }));
afterEach(() => {
  cleanup();
  document.documentElement.classList.remove("flat-theme");
  vi.clearAllMocks();
});

describe("management preferences in the browser", () => {
  it("rejects invalid values", () => {
    expect(parsePreferences({ appearance: "invalid", themeMode: "<script>", density: "huge" }))
      .toEqual({ appearance: null, themeMode: null, density: null });
  });
  it("loads server choices and applies later MCP changes", async () => {
    fetch.mockResolvedValue({ appearance: "light", themeMode: "flat", density: "compact" });
    const appearance = vi.fn();
    const mode = vi.fn();
    renderHook(() => useServerPreferences(appearance, mode));
    await waitFor(() => expect(appearance).toHaveBeenCalledWith("light"));
    expect(document.documentElement).toHaveClass("flat-theme");
    expect(document.documentElement.style.getPropertyValue("--panel-inset")).toBe("6px");
    act(() => publishPreferences({ appearance: "dark", themeMode: "glassy", density: "roomy" }));
    expect(appearance).toHaveBeenLastCalledWith("dark");
    expect(mode).toHaveBeenLastCalledWith("glassy");
    expect(document.documentElement).not.toHaveClass("flat-theme");
    expect(document.documentElement.style.getPropertyValue("--panel-inset")).toBe("20px");
  });
  it("does not let a slow startup request overwrite a newer MCP update", async () => {
    let resolve: (value: unknown) => void = () => {};
    fetch.mockImplementation(() => new Promise(done => { resolve = done; }));
    const appearance = vi.fn();
    const mode = vi.fn();
    renderHook(() => useServerPreferences(appearance, mode));
    act(() => publishPreferences({ appearance: "dark" }));
    await act(async () => resolve({ appearance: "light" }));
    expect(appearance).toHaveBeenCalledTimes(1);
    expect(appearance).toHaveBeenLastCalledWith("dark");
  });
  it("keeps existing browser choices when the server has none", async () => {
    fetch.mockResolvedValue({ appearance: null, themeMode: null, density: null });
    const appearance = vi.fn();
    const mode = vi.fn();
    renderHook(() => useServerPreferences(appearance, mode));
    await act(async () => {});
    expect(appearance).not.toHaveBeenCalled();
    expect(mode).not.toHaveBeenCalled();
  });
});
