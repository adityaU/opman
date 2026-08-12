import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { EditorSection } from "../settings-page/EditorSection";

describe("EditorSection", () => {
  beforeEach(() => {
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: () => ({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      }),
    });
  });

  it("renders both choices and marks the current engine", () => {
    render(<EditorSection engine="neovim" onEngineChange={vi.fn()} />);
    const options = screen.getAllByRole("radio");

    expect(options).toHaveLength(2);
    expect(options[0]).toHaveAttribute("aria-checked", "false");
    expect(options[1]).toHaveAttribute("aria-checked", "true");
    expect(screen.getByText("Neovim motions, operators, and registers drive the same editor.")).toBeTruthy();
  });

  it("calls the setter when the user chooses an engine", () => {
    const onEngineChange = vi.fn();
    render(<EditorSection engine="codemirror" onEngineChange={onEngineChange} />);
    fireEvent.click(screen.getByRole("radio", { name: /Neovim/ }));
    expect(onEngineChange).toHaveBeenCalledWith("neovim");
  });
});
