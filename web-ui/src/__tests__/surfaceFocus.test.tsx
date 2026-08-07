import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_CONFIG } from "../keybindings/config";
import { KeymapProvider } from "../keybindings/KeymapContext";
import { useCommands } from "../keybindings/useCommand";
import { useKeymapListener } from "../keybindings/useKeymapListener";
import { useSurfaceFocus } from "../keybindings/useSurfaceFocus";
import type { Host } from "../keybindings/types";

const HOST: Host = { platform: "linux", target: "web", browser: "chrome" };

function Runtime() {
  useKeymapListener();
  useSurfaceFocus();
  return null;
}

interface Props {
  readonly onNewFile: () => void;
  readonly onStage: () => void;
}

/**
 * Two panels marked the way the real ones are, so the test exercises the same
 * `data-surface` path the app uses rather than a stand-in.
 */
function Panels({ onNewFile, onStage }: Props) {
  useCommands({ "explorer.newFile": onNewFile, "git.stageFile": onStage });
  return (
    <>
      <div data-surface="explorer">
        <button type="button" data-testid="explorer-btn">
          explorer
        </button>
      </div>
      <div data-surface="git">
        <button type="button" data-testid="git-btn">
          git
        </button>
      </div>
    </>
  );
}

function mount(props: Props, mode: "normal" | "vim" = "vim") {
  return render(
    <KeymapProvider config={{ ...DEFAULT_CONFIG, mode }} host={HOST}>
      <Runtime />
      <Panels {...props} />
    </KeymapProvider>,
  );
}

function press(key: string) {
  act(() => {
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key, bubbles: true }),
    );
  });
}

describe("surface focus", () => {
  it("routes a bare key to the focused surface", async () => {
    const onNewFile = vi.fn();
    const onStage = vi.fn();
    mount({ onNewFile, onStage });

    await userEvent.click(screen.getByTestId("explorer-btn"));
    press("a");

    expect(onNewFile).toHaveBeenCalledTimes(1);
    expect(onStage).not.toHaveBeenCalled();
  });

  it("stops routing it once another surface takes focus", async () => {
    const onNewFile = vi.fn();
    const onStage = vi.fn();
    mount({ onNewFile, onStage });

    await userEvent.click(screen.getByTestId("git-btn"));
    press("a");
    expect(onNewFile).not.toHaveBeenCalled();

    // `s` stages the selected file in the git panel, and is unbound in the explorer.
    press("s");
    expect(onStage).toHaveBeenCalledTimes(1);
  });

  it("follows focus back and forth", async () => {
    const onNewFile = vi.fn();
    const onStage = vi.fn();
    mount({ onNewFile, onStage });

    await userEvent.click(screen.getByTestId("explorer-btn"));
    press("a");
    await userEvent.click(screen.getByTestId("git-btn"));
    press("s");
    await userEvent.click(screen.getByTestId("explorer-btn"));
    press("a");

    expect(onNewFile).toHaveBeenCalledTimes(2);
    expect(onStage).toHaveBeenCalledTimes(1);
  });

  it("leaves bare keys alone in normal mode", async () => {
    const onNewFile = vi.fn();
    const onStage = vi.fn();
    mount({ onNewFile, onStage }, "normal");

    await userEvent.click(screen.getByTestId("explorer-btn"));
    press("a");

    expect(onNewFile).not.toHaveBeenCalled();
  });
});
