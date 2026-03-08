/**
 * Unit tests for useFocusTrap hook.
 */
import { describe, it, expect, vi, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import React from "react";
import { useFocusTrap } from "../hooks/useFocusTrap";

afterEach(() => {
  vi.restoreAllMocks();
  // Clean up any appended DOM elements
  document.body.innerHTML = "";
});

function createContainer(): HTMLDivElement {
  const container = document.createElement("div");
  const btn1 = document.createElement("button");
  btn1.textContent = "First";
  const btn2 = document.createElement("button");
  btn2.textContent = "Second";
  const btn3 = document.createElement("button");
  btn3.textContent = "Third";
  container.append(btn1, btn2, btn3);
  document.body.appendChild(container);
  return container;
}

describe("useFocusTrap", () => {
  it("focuses the first focusable element on mount", () => {
    const container = createContainer();
    const ref = { current: container } as React.RefObject<HTMLElement>;
    renderHook(() => useFocusTrap(ref));
    expect(document.activeElement).toBe(container.querySelector("button"));
  });

  it("restores focus on unmount", () => {
    // Create an external element to hold focus before mounting
    const external = document.createElement("button");
    external.textContent = "External";
    document.body.appendChild(external);
    external.focus();
    expect(document.activeElement).toBe(external);

    const container = createContainer();
    const ref = { current: container } as React.RefObject<HTMLElement>;
    const { unmount } = renderHook(() => useFocusTrap(ref));

    // Focus should have moved into the container
    expect(document.activeElement).toBe(container.querySelector("button"));

    // Unmount should restore focus back to the external button
    unmount();
    expect(document.activeElement).toBe(external);
  });

  it("wraps Tab from last to first element", () => {
    const container = createContainer();
    const buttons = container.querySelectorAll("button");
    const ref = { current: container } as React.RefObject<HTMLElement>;
    renderHook(() => useFocusTrap(ref));

    // Focus the last button
    buttons[2].focus();
    expect(document.activeElement).toBe(buttons[2]);

    // Press Tab — should wrap to the first button
    const event = new KeyboardEvent("keydown", {
      key: "Tab",
      bubbles: true,
      cancelable: true,
    });
    const spy = vi.spyOn(event, "preventDefault");
    document.dispatchEvent(event);
    expect(spy).toHaveBeenCalled();
    expect(document.activeElement).toBe(buttons[0]);
  });

  it("wraps Shift+Tab from first to last element", () => {
    const container = createContainer();
    const buttons = container.querySelectorAll("button");
    const ref = { current: container } as React.RefObject<HTMLElement>;
    renderHook(() => useFocusTrap(ref));

    // Focus should start at first button
    expect(document.activeElement).toBe(buttons[0]);

    // Press Shift+Tab — should wrap to the last button
    const event = new KeyboardEvent("keydown", {
      key: "Tab",
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });
    const spy = vi.spyOn(event, "preventDefault");
    document.dispatchEvent(event);
    expect(spy).toHaveBeenCalled();
    expect(document.activeElement).toBe(buttons[2]);
  });

  it("does nothing with null ref", () => {
    const ref = { current: null } as React.RefObject<HTMLElement | null>;
    // Should not throw
    expect(() => {
      renderHook(() => useFocusTrap(ref));
    }).not.toThrow();
  });
});
