import { act, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_CONFIG } from "../keybindings/config";
import { KeymapProvider } from "../keybindings/KeymapContext";
import { useKeymapListener } from "../keybindings/useKeymapListener";
import { useWhenContext } from "../keybindings/useCommand";
import { PendingChordStrip, WhichKeyPanel } from "../keybindings/which-key/WhichKeyPanel";
import type { Host, Mode } from "../keybindings/types";
import type { WhichKeyConfig } from "../keybindings/config";

const HOST: Host = { platform: "linux", target: "web", browser: "chrome" };

type Ctx = Record<string, string | boolean>;

function Harness({ context }: { context: Ctx }) {
  const listener = useKeymapListener();
  useWhenContext(context);
  return (
    <>
      <WhichKeyPanel listener={listener} />
      <PendingChordStrip listener={listener} />
    </>
  );
}

/**
 * A repo is open by default, because hints are filtered by `when` and a bare
 * context would hide most of the tree — which is correct behaviour but makes
 * for a test that proves nothing.
 */
function mount(mode: Mode = "vim", whichKey?: Partial<WhichKeyConfig>, context: Ctx = { gitRepo: true }) {
  return render(
    <KeymapProvider
      config={{ ...DEFAULT_CONFIG, mode, whichKey: { ...DEFAULT_CONFIG.whichKey, ...whichKey } }}
      host={HOST}
    >
      <Harness context={context} />
    </KeymapProvider>,
  );
}

/** Dispatch a key the way the capture-phase listener will see it. */
function press(key: string, init: Partial<KeyboardEventInit> = {}) {
  act(() => {
    document.body.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, ...init }));
  });
}

function advance(ms: number) {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

function withFakeTimers(run: () => void) {
  vi.useFakeTimers();
  try {
    run();
  } finally {
    vi.useRealTimers();
  }
}

describe("WhichKeyPanel", () => {
  it("stays hidden until the delay elapses", () => {
    withFakeTimers(() => {
      mount();
      press(" ");
      expect(screen.queryByRole("dialog")).toBeNull();

      advance(DEFAULT_CONFIG.whichKey.delayMs + 10);
      expect(screen.getByRole("dialog")).toBeTruthy();
    });
  });

  it("never appears for a chord completed quickly", () => {
    withFakeTimers(() => {
      mount();
      press(" ");
      advance(50);
      press("g");
      advance(50);
      press("g");
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  // `?` reveals the hints only where the keymap does not want it. At the
  // leader level `<leader>?` is bound to Help, and a real binding must win.
  it("appears immediately when ? is pressed on a level that does not bind it", () => {
    withFakeTimers(() => {
      mount();
      press(" ");
      press("g");
      press("?");
      expect(screen.getByRole("dialog")).toBeTruthy();
    });
  });

  it("lets a bound ? win over the reveal convenience", () => {
    withFakeTimers(() => {
      mount();
      press(" ");
      press("?");
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  it("lists namespaces as prefixes and shows the pending chord", () => {
    withFakeTimers(() => {
      mount();
      press(" ");
      advance(DEFAULT_CONFIG.whichKey.delayMs + 10);

      expect(screen.getByText("+git")).toBeTruthy();
      expect(screen.getByText("+sessions")).toBeTruthy();
      expect(document.querySelector(".which-key-crumb")?.textContent).toBe("Space");
    });
  });

  it("descends into a namespace and lists its leaves", () => {
    withFakeTimers(() => {
      mount();
      press(" ");
      advance(DEFAULT_CONFIG.whichKey.delayMs + 10);
      press("g");
      advance(DEFAULT_CONFIG.whichKey.delayMs + 10);

      expect(screen.getByText("branch")).toBeTruthy();
      expect(screen.getByText("panel")).toBeTruthy();
      expect(screen.queryByText("+git")).toBeNull();
    });
  });

  it("hides a leaf whose when clause is false", () => {
    withFakeTimers(() => {
      mount("vim", undefined, {});
      press(" ");
      press("g");
      press("?");

      expect(screen.getByText("panel")).toBeTruthy();
      expect(screen.queryByText("branch")).toBeNull();
    });
  });

  it("ascends on Backspace", () => {
    withFakeTimers(() => {
      mount();
      press(" ");
      press("g");
      press("?");
      expect(screen.getByText("branch")).toBeTruthy();

      // Revealing survives the ascent: the panel stays open one level up.
      press("Backspace");
      expect(screen.getByText("+git")).toBeTruthy();
      expect(screen.queryByText("branch")).toBeNull();
    });
  });

  it("cancels on Escape without running the global Escape command", () => {
    withFakeTimers(() => {
      mount();
      press(" ");
      press("g");
      press("?");
      expect(screen.getByRole("dialog")).toBeTruthy();

      press("Escape");
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  it("honours the enabled flag", () => {
    withFakeTimers(() => {
      mount("vim", { enabled: false });
      press(" ");
      advance(DEFAULT_CONFIG.whichKey.delayMs + 10);
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  it("respects a custom delay", () => {
    withFakeTimers(() => {
      mount("vim", { delayMs: 1000 });
      press(" ");
      advance(500);
      expect(screen.queryByRole("dialog")).toBeNull();
      advance(600);
      expect(screen.getByRole("dialog")).toBeTruthy();
    });
  });
});

describe("PendingChordStrip", () => {
  it("shows the pending chord in normal mode", () => {
    withFakeTimers(() => {
      mount("normal");
      press("k", { ctrlKey: true });
      expect(screen.getByRole("status").textContent).toContain("Ctrl+K");
    });
  });

  it("stays out of the way in vim mode, where the panel does the work", () => {
    withFakeTimers(() => {
      mount("vim");
      press("k", { ctrlKey: true });
      expect(screen.queryByRole("status")).toBeNull();
    });
  });
});
