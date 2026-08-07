/**
 * The two guards the directional keys live or die by.
 *
 * `Ctrl+H` carries a modifier, so the matcher's insert guard would let it
 * through a focused textarea by default — and there it is Backspace to
 * readline and to every shell in the terminal widget. `!textInput` is what
 * takes it back, and it is worth a test of its own because the failure is
 * silent: the key still moves focus, it just also eats a character.
 */
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_CONFIG } from "../keybindings/config";
import { KeymapProvider } from "../keybindings/KeymapContext";
import { useCommands } from "../keybindings/useCommand";
import { useKeymapListener } from "../keybindings/useKeymapListener";
import { COMMANDS } from "../keybindings/commands";
import { builtInLayers } from "../keybindings/layers";
import { resolve } from "../keybindings/resolve";
import type { Host } from "../keybindings/types";

const LINUX: Host = { platform: "linux", target: "web", browser: "chrome" };
const MAC: Host = { platform: "mac", target: "web", browser: "chrome" };

function Listener() {
  useKeymapListener();
  return null;
}

function Harness({ onCommand }: { readonly onCommand: Record<string, () => void> }) {
  return (
    <KeymapProvider config={DEFAULT_CONFIG} host={LINUX}>
      <Listener />
      <Surface onCommand={onCommand} />
    </KeymapProvider>
  );
}

function Surface({ onCommand }: { readonly onCommand: Record<string, () => void> }) {
  useCommands(onCommand);
  return (
    <>
      <textarea data-testid="composer" />
      <button data-testid="plain">plain</button>
    </>
  );
}

const chordsFor = (host: Host, command: string) =>
  resolve(builtInLayers(), { host, mode: "normal" })
    .bindings.filter((b) => b.command === command)
    .map((b) => b.id);

describe("directional focus keys", () => {
  it("moves focus from outside a text field", async () => {
    const left = vi.fn();
    render(<Harness onCommand={{ "nav.focusLeft": left }} />);
    screen.getByTestId("plain").focus();

    await userEvent.keyboard("{Control>}h{/Control}");
    expect(left).toHaveBeenCalledTimes(1);
  });

  it("stays out of the way while a textarea has focus", async () => {
    const left = vi.fn();
    const down = vi.fn();
    render(<Harness onCommand={{ "nav.focusLeft": left, "nav.focusDown": down }} />);
    const composer = screen.getByTestId("composer");
    composer.focus();

    await userEvent.keyboard("{Control>}h{/Control}");
    await userEvent.keyboard("{Control>}j{/Control}");
    expect(left).not.toHaveBeenCalled();
    expect(down).not.toHaveBeenCalled();
  });

  it("leaves Ctrl+arrow to the text field too, so word motion survives", async () => {
    const right = vi.fn();
    render(<Harness onCommand={{ "nav.focusRight": right }} />);
    screen.getByTestId("composer").focus();

    await userEvent.keyboard("{Control>}{ArrowRight}{/Control}");
    expect(right).not.toHaveBeenCalled();
  });

  it("still moves on Ctrl+arrow outside one", async () => {
    const right = vi.fn();
    render(<Harness onCommand={{ "nav.focusRight": right }} />);
    screen.getByTestId("plain").focus();

    await userEvent.keyboard("{Control>}{ArrowRight}{/Control}");
    expect(right).toHaveBeenCalledTimes(1);
  });
});

describe("the Ctrl+K exception", () => {
  it("takes ctrl+k for Focus Up only where it is not the chord prefix", () => {
    // On macOS `mod` is Command, so ctrl+k is free; elsewhere it opens
    // `mod+k mod+t`, `mod+k d` and the rest, and the matcher would wait for a
    // second step instead of firing.
    expect(chordsFor(MAC, "nav.focusUp")).toContain("ctrl+k");
    expect(chordsFor(LINUX, "nav.focusUp")).not.toContain("ctrl+k");
  });

  it("keeps the direction reachable on every platform", () => {
    for (const host of [MAC, LINUX]) {
      expect(chordsFor(host, "nav.focusUp")).toContain("ctrl+up");
    }
  });

  it("registers every new command with a title and a category", () => {
    const ids = ["nav.focusLeft", "nav.focusRight", "nav.focusUp", "nav.focusDown"];
    for (const id of [...ids, "sidebar.moveDown", "sidebar.open"]) {
      const command = COMMANDS.find((c) => c.id === id);
      expect(command, id).toBeDefined();
      expect(command?.title).toBeTruthy();
      expect(command?.category).toBeTruthy();
    }
  });
});

describe("bare list keys", () => {
  it("scopes the sidebar's letters to the focused sidebar", () => {
    const sidebar = resolve(builtInLayers(), { host: LINUX, mode: "normal" }).bindings.filter(
      (b) => b.command.startsWith("sidebar."),
    );
    expect(sidebar.length).toBeGreaterThan(0);
    for (const binding of sidebar) expect(binding.when).toBe("focus==sidebar");
  });

  it("gives the explorer hjkl in normal mode, not just vim", () => {
    const chords = resolve(builtInLayers(), { host: LINUX, mode: "normal" })
      .bindings.filter((b) => b.command === "explorer.moveDown")
      .map((b) => b.id);
    expect(chords).toEqual(expect.arrayContaining(["down", "j"]));
  });
});
