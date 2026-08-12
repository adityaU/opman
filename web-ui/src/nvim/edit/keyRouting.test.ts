import { describe, expect, it, vi } from "vitest";
import { routeNvimKey } from "./keyRouting";

describe("Neovim key routing", () => {
  it("routes normal-mode keys through the shared encoder", () => {
    const input = vi.fn();
    const event = new KeyboardEvent("keydown", { key: "g", code: "KeyG" });

    expect(routeNvimKey(event, "normal", input, vi.fn())).toBe(true);
    expect(input).toHaveBeenCalledWith("g");
  });

  it("keeps insert-mode plain keys local", () => {
    const input = vi.fn();
    const event = new KeyboardEvent("keydown", { key: "a", code: "KeyA" });

    expect(routeNvimKey(event, "insert", input, vi.fn())).toBe(false);
    expect(input).not.toHaveBeenCalled();
  });

  it("routes insert-mode modifier chords", () => {
    const input = vi.fn();
    const event = new KeyboardEvent("keydown", { key: "r", code: "KeyR", ctrlKey: true });

    expect(routeNvimKey(event, "insert", input, vi.fn())).toBe(true);
    expect(input).toHaveBeenCalled();
  });

  it("leaves insert-mode Ctrl+V to the browser clipboard", () => {
    const input = vi.fn();
    const event = new KeyboardEvent("keydown", { key: "v", code: "KeyV", ctrlKey: true });

    expect(routeNvimKey(event, "insert", input, vi.fn())).toBe(false);
    expect(input).not.toHaveBeenCalled();
  });

  it("routes normal-mode Ctrl+V to Neovim", () => {
    const input = vi.fn();
    const event = new KeyboardEvent("keydown", { key: "v", code: "KeyV", ctrlKey: true });

    expect(routeNvimKey(event, "normal", input, vi.fn())).toBe(true);
    expect(input).toHaveBeenCalled();
  });

  it("leaves normal-mode Meta+C to the browser clipboard", () => {
    const clipboardInput = vi.fn();
    const event = new KeyboardEvent("keydown", { key: "c", code: "KeyC", metaKey: true });

    expect(routeNvimKey(event, "normal", clipboardInput, vi.fn())).toBe(false);
    expect(clipboardInput).not.toHaveBeenCalled();
  });
});
