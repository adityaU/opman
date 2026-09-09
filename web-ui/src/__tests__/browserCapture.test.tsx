/**
 * A focused browser pane owns the keyboard.
 *
 * The app keymap listens in the capture phase, so without an explicit stand-down
 * a page in a pane cannot be typed into: `/` opens the palette, `g` starts a
 * chord, and the site never sees either. These tests pin both halves — the
 * keymap stands down while the surface has focus, and the double Escape that
 * gives the keyboard back.
 */
import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_CONFIG } from "../keybindings/config";
import { KeymapProvider } from "../keybindings/KeymapContext";
import { useCommands } from "../keybindings/useCommand";
import { useKeymapListener } from "../keybindings/useKeymapListener";
import {
  BROWSER_CAPTURE_ATTRIBUTE,
  RELEASE_WINDOW_MS,
  browserOwnsKey,
  releasesKeyboard,
} from "../browser-panel/capture";
import type { Host } from "../keybindings/types";

const HOST: Host = { platform: "linux", target: "web", browser: "chrome" };

function Listener() {
  useKeymapListener();
  return null;
}

function Harness({ onCommand }: { readonly onCommand: Record<string, () => void> }) {
  return (
    <KeymapProvider config={DEFAULT_CONFIG} host={HOST}>
      <Listener />
      <Commands onCommand={onCommand} />
      {/* The mark is permanent on the surface; only where focus sits decides. */}
      <div data-testid="page" tabIndex={-1} {...{ [BROWSER_CAPTURE_ATTRIBUTE]: "" }} />
      <div data-testid="elsewhere" tabIndex={-1} />
    </KeymapProvider>
  );
}

function Commands({ onCommand }: { readonly onCommand: Record<string, () => void> }) {
  useCommands(onCommand);
  return null;
}

describe("browser pane keyboard capture", () => {
  it("stands the app keymap down while the page has focus", async () => {
    const toggleSidebar = vi.fn();
    const { getByTestId } = render(
      <Harness onCommand={{ "layout.toggleSidebar": toggleSidebar }} />,
    );
    getByTestId("page").focus();

    await userEvent.keyboard("{Control>}b{/Control}");
    expect(toggleSidebar).not.toHaveBeenCalled();
  });

  it("leaves the keymap alone once focus is anywhere else", async () => {
    // A browser pane in the background must not mute chords for the whole app.
    const toggleSidebar = vi.fn();
    const { getByTestId } = render(
      <Harness onCommand={{ "layout.toggleSidebar": toggleSidebar }} />,
    );
    getByTestId("elsewhere").focus();

    await userEvent.keyboard("{Control>}b{/Control}");
    expect(toggleSidebar).toHaveBeenCalledTimes(1);
  });

  it("claims keys only for elements inside the surface", () => {
    const surface = document.createElement("div");
    surface.setAttribute(BROWSER_CAPTURE_ATTRIBUTE, "");
    const inside = document.createElement("span");
    surface.append(inside);
    const outside = document.createElement("div");

    expect(browserOwnsKey(surface)).toBe(true);
    expect(browserOwnsKey(inside)).toBe(true);
    expect(browserOwnsKey(outside)).toBe(false);
    expect(browserOwnsKey(null)).toBe(false);
  });
});

describe("the way back to the app", () => {
  it("releases only on a second Escape in quick succession", () => {
    // A lone Escape belongs to the page: sites close their own dialogs with it.
    expect(releasesKeyboard("Escape", undefined, 1_000)).toBe(false);
    expect(releasesKeyboard("Escape", 1_000, 1_000 + RELEASE_WINDOW_MS)).toBe(true);
  });

  it("does not release on two Escapes far apart", () => {
    expect(releasesKeyboard("Escape", 1_000, 1_000 + RELEASE_WINDOW_MS + 1)).toBe(false);
  });

  it("never releases on any other key", () => {
    expect(releasesKeyboard("Enter", 1_000, 1_010)).toBe(false);
    expect(releasesKeyboard("q", 1_000, 1_010)).toBe(false);
  });
});
