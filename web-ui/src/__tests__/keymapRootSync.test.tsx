import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_CONFIG } from "../keybindings/config";
import { useKeymapContext } from "../keybindings/KeymapContext";
import { KeymapRoot } from "../keybindings/KeymapRoot";

/**
 * The app-level config follows an edit made elsewhere.
 *
 * `KeymapRoot` fetches once at mount, so before this it held whatever the app started
 * with for the rest of the session. That is the bug behind "the keybindings page saves the
 * mode but does not show it": the page reads the active mode back from here.
 */

vi.mock("../api/keybindings", async () => {
  const actual = await vi.importActual<typeof import("../api/keybindings")>(
    "../api/keybindings",
  );
  return {
    ...actual,
    loadKeybindingsOrDefault: vi.fn(async () => ({
      config: { ...DEFAULT_CONFIG, mode: "normal" as const },
      diagnostics: [],
      path: "/home/u/.config/opman/keybindings.json",
    })),
  };
});

const { KEYBINDINGS_CHANGED, publishKeybindings } = await import("../api/keybindings");

function Readout() {
  const { mode, config } = useKeymapContext();
  return (
    <>
      <span data-testid="mode">{mode}</span>
      <span data-testid="leader">{config.leader}</span>
    </>
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("KeymapRoot", () => {
  it("starts on the fetched config", async () => {
    render(
      <KeymapRoot>
        <Readout />
      </KeymapRoot>,
    );

    await waitFor(() => expect(screen.getByTestId("mode").textContent).toBe("normal"));
  });

  it("adopts a published config without a reload", async () => {
    render(
      <KeymapRoot>
        <Readout />
      </KeymapRoot>,
    );
    await waitFor(() => expect(screen.getByTestId("mode").textContent).toBe("normal"));

    act(() => publishKeybindings({ ...DEFAULT_CONFIG, mode: "vim" }));

    expect(screen.getByTestId("mode").textContent).toBe("vim");
  });

  /* The mode is the reported symptom, but every edit travels the same way — a rebound
     chord that never reaches here is a key the user set and cannot press. */
  it("adopts the whole config, not just the mode", async () => {
    render(
      <KeymapRoot>
        <Readout />
      </KeymapRoot>,
    );
    await waitFor(() => expect(screen.getByTestId("mode").textContent).toBe("normal"));

    act(() => publishKeybindings({ ...DEFAULT_CONFIG, leader: "," }));

    expect(screen.getByTestId("leader").textContent).toBe(",");
  });

  it("stops listening once it is unmounted", async () => {
    const { unmount } = render(
      <KeymapRoot>
        <Readout />
      </KeymapRoot>,
    );
    await waitFor(() => expect(screen.getByTestId("mode").textContent).toBe("normal"));
    unmount();

    // No listener, so no state update on an unmounted tree — this throwing or warning is
    // the failure being guarded against.
    expect(() =>
      window.dispatchEvent(
        new CustomEvent(KEYBINDINGS_CHANGED, { detail: { ...DEFAULT_CONFIG, mode: "vim" } }),
      ),
    ).not.toThrow();
  });
});
