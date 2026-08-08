import { describe, expect, it } from "vitest";
import { COMMANDS, findCommand } from "../keybindings/commands";
import { validate } from "../keybindings/conflicts";
import { builtInLayers } from "../keybindings/layers";
import { resolve } from "../keybindings/resolve";
import type { Browser, Conflict, Host, Mode, Platform, Target } from "../keybindings/types";

const PLATFORMS: readonly Platform[] = ["mac", "win", "linux"];
const TARGETS: readonly Target[] = ["web", "desktop"];
const BROWSERS: readonly Browser[] = ["chrome", "firefox", "safari"];
const MODES: readonly Mode[] = ["normal", "vim"];

function combinations(): { host: Host; mode: Mode }[] {
  const out: { host: Host; mode: Mode }[] = [];
  for (const platform of PLATFORMS) {
    for (const target of TARGETS) {
      for (const browser of BROWSERS) {
        for (const mode of MODES) out.push({ host: { platform, target, browser }, mode });
      }
    }
  }
  return out;
}

function describeConflicts(conflicts: readonly Conflict[]): string {
  return conflicts.map((c) => `[${c.kind}] ${c.chord} — ${c.detail}`).join("\n");
}

function compose(host: Host, mode: Mode) {
  const { bindings, rejected } = resolve(builtInLayers(), { host, mode });
  return { bindings, rejected, conflicts: validate({ bindings, host, commands: COMMANDS }) };
}

describe("keymap matrix", () => {
  const cases = combinations();

  it("covers every platform, target, browser and mode", () => {
    expect(cases).toHaveLength(PLATFORMS.length * TARGETS.length * BROWSERS.length * MODES.length);
  });

  it.each(cases)("resolves cleanly for $host.platform/$host.target/$host.browser in $mode", ({ host, mode }) => {
    const { bindings, rejected, conflicts } = compose(host, mode);
    expect(rejected, describeConflicts(rejected)).toHaveLength(0);
    expect(conflicts, describeConflicts(conflicts)).toHaveLength(0);
    expect(bindings.length).toBeGreaterThan(100);
  });
});

describe("command registry", () => {
  it("has no duplicate ids", () => {
    const seen = new Set<string>();
    const duplicates = COMMANDS.filter((c) => !seen.add(c.id) && true);
    expect(duplicates.map((c) => c.id)).toEqual([]);
  });

  // The point is reachability, not chords: a command may be reached by a key, by the
  // palette, or by typing its slash in the composer, but a command reachable by none of
  // the three is dead code that still shows up in the keybindings view.
  it("makes every command reachable", () => {
    const host: Host = { platform: "linux", target: "web", browser: "chrome" };
    const bound = new Set<string>();
    for (const mode of MODES) {
      for (const binding of compose(host, mode).bindings) bound.add(binding.command);
    }
    const unreachable = COMMANDS.filter(
      (c) => !c.paletteOnly && !c.slash && !bound.has(c.id),
    ).map((c) => c.id);
    expect(unreachable).toEqual([]);
  });

  // Which-key only ever renders the continuations of a prefix, so a binding
  // needs a label exactly when it has one — single-key bindings are never listed.
  it("gives every prefixed vim binding a which-key group and label", () => {
    const host: Host = { platform: "mac", target: "web", browser: "chrome" };
    const missing = compose(host, "vim")
      .bindings.filter((b) => b.mode === "vim" && b.seq.length > 1)
      .filter((b) => !b.group || !(b.label ?? findCommand(b.command)?.label))
      .map((b) => `${b.id} → ${b.command}`);
    expect(missing).toEqual([]);
  });
});

describe("target differences", () => {
  const mac = { platform: "mac", browser: "chrome" } as const;

  it("keeps the canonical chord on desktop and moves it on web", () => {
    const desktop = compose({ ...mac, target: "desktop" }, "normal").bindings;
    const web = compose({ ...mac, target: "web" }, "normal").bindings;

    const chordFor = (list: typeof web, command: string) =>
      list.filter((b) => b.command === command).map((b) => b.id);

    expect(chordFor(desktop, "session.new")).toEqual(["meta+n"]);
    expect(chordFor(web, "session.new")).toEqual(["alt+meta+n"]);
  });

  it("drops the Firefox-stolen palette chord but keeps F1", () => {
    const firefox = compose({ platform: "win", target: "web", browser: "firefox" }, "normal").bindings;
    const chords = firefox.filter((b) => b.command === "palette.commands").map((b) => b.id);
    expect(chords).toContain("f1");
    expect(chords).not.toContain("ctrl+shift+p");
  });

  it("keeps the palette chord in Chrome", () => {
    const chrome = compose({ platform: "win", target: "web", browser: "chrome" }, "normal").bindings;
    const chords = chrome.filter((b) => b.command === "palette.commands").map((b) => b.id);
    expect(chords).toEqual(expect.arrayContaining(["f1", "ctrl+shift+p"]));
  });
});
