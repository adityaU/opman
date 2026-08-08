// Node's types are pulled in here and nowhere else: this test reads the source
// tree from disk (far faster than routing 400 files through Vite), and app code
// has no business seeing `process` or `Buffer`.
/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { displayChord, parseChord } from "../keybindings/chord";
import { COMMANDS } from "../keybindings/commands";
import { DEFAULT_CONFIG } from "../keybindings/config";
import { KeymapProvider } from "../keybindings/KeymapContext";
import { KeyHint } from "../keybindings/hint/KeyHint";
import { place } from "../keybindings/hint/placement";
import { preferredBinding } from "../keybindings/useChord";
import type { Host, Mode, ResolvedBinding } from "../keybindings/types";

/**
 * The shortcut a control advertises has to be the one that would actually fire
 * if the user pressed it — same platform, same mode, same config. These are the
 * three ways that used to go wrong.
 */

const HOST: Host = { platform: "linux", target: "web", browser: "chrome" };

function mount(mode: Mode, node: React.ReactNode) {
  return render(
    <KeymapProvider config={{ ...DEFAULT_CONFIG, mode }} host={HOST}>
      {node}
    </KeymapProvider>,
  );
}

function binding(over: Partial<ResolvedBinding>): ResolvedBinding {
  return {
    seq: parseChord("ctrl+b"),
    id: "ctrl+b",
    command: "layout.toggleSidebar",
    source: "base",
    ...over,
  };
}

describe("displayChord", () => {
  it("spells named keys as they read on the cap", () => {
    expect(displayChord(parseChord("ctrl+shift+`"), "linux")).toBe("Ctrl+Shift+`");
    expect(displayChord(parseChord("<space>"), "linux")).toBe("Space");
    expect(displayChord(parseChord("alt+down"), "linux")).toBe("Alt+↓");
    expect(displayChord(parseChord("f2"), "linux")).toBe("F2");
    expect(displayChord(parseChord("delete"), "linux")).toBe("Del");
  });

  // A vim binding written `]h` must not be shown as "] H": that names two keys
  // the user's own config does not contain.
  it("renders a vim literal run the way vim writes it", () => {
    expect(displayChord(parseChord("]h"), "linux", "vim")).toBe("]h");
    expect(displayChord(parseChord("<space>ot"), "linux", "vim")).toBe("Space ot");
    expect(displayChord(parseChord("<space>gS"), "linux", "vim")).toBe("Space gS");
  });

  it("keeps modifier chords in modifier notation, in either mode", () => {
    expect(displayChord(parseChord("ctrl+\\ ctrl+o"), "linux", "vim")).toBe("Ctrl+\\ Ctrl+O");
    expect(displayChord(parseChord("shift+meta+p"), "mac", "vim")).toBe("⇧⌘P");
  });
});

describe("preferredBinding", () => {
  it("advertises the chord of the mode the user chose", () => {
    const shared = binding({ id: "ctrl+`", command: "layout.toggleTerminal" });
    const vim = binding({ id: "space o t", command: "layout.toggleTerminal", mode: "vim" });
    expect(preferredBinding([shared, vim], "vim")).toBe(vim);
    expect(preferredBinding([shared, vim], "normal")).toBe(shared);
  });

  it("prefers a chord that always works over one scoped to a surface", () => {
    const scoped = binding({ id: "f5", when: "focus==git" });
    const global = binding({ id: "ctrl+b" });
    expect(preferredBinding([scoped, global], "normal")).toBe(global);
  });
});

describe("KeyHint", () => {
  it("shows the label and the live chord on hover, and clears on leave", () => {
    mount("normal", (
      <KeyHint label="Toggle sidebar" command="layout.toggleSidebar">
        <button>sidebar</button>
      </KeyHint>
    ));

    const trigger = screen.getByText("sidebar");
    act(() => {
      fireEvent.pointerEnter(trigger);
    });
    expect(screen.queryByRole("tooltip")).toBeNull();

    act(() => {
      fireEvent.focus(trigger);
    });
    const tip = screen.getByRole("tooltip");
    expect(tip.textContent).toContain("Toggle sidebar");
    expect(tip.textContent).toContain("Ctrl+B");

    act(() => {
      fireEvent.blur(trigger);
    });
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  // The same button, in vim mode, must name the leader sequence — not the
  // Ctrl chord that also happens to still work.
  it("names the vim chord when the keymap is in vim mode", () => {
    mount("vim", (
      <KeyHint label="Toggle sidebar" command="layout.toggleSidebar">
        <button>sidebar</button>
      </KeyHint>
    ));
    act(() => {
      fireEvent.focus(screen.getByText("sidebar"));
    });
    // One cap per press, so the leader and its two letters read as two keys.
    const keys = [...document.querySelectorAll(".khint-key")].map((k) => k.textContent);
    expect(keys).toEqual(["Space", "wb"]);
  });

  it("publishes the chord to assistive tech, and shows the label alone when unbound", () => {
    mount("normal", (
      <KeyHint label="Reload application" command="system.refreshApp">
        <button>reload</button>
      </KeyHint>
    ));
    const trigger = screen.getByText("reload");
    expect(trigger.getAttribute("aria-keyshortcuts")).toBeNull();
    act(() => {
      fireEvent.focus(trigger);
    });
    expect(screen.getByRole("tooltip").textContent).toBe("Reload application");
  });

  it("keeps the child's own handlers", () => {
    let clicks = 0;
    mount("normal", (
      <KeyHint label="Toggle sidebar" command="layout.toggleSidebar">
        <button onClick={() => (clicks += 1)}>sidebar</button>
      </KeyHint>
    ));
    fireEvent.click(screen.getByText("sidebar"));
    expect(clicks).toBe(1);
  });
});

describe("place", () => {
  const viewport = { width: 1000, height: 800 };
  const tip = { top: 0, left: 0, width: 200, height: 40 };

  it("sits below and centred when there is room", () => {
    const anchor = { top: 100, left: 400, width: 40, height: 24 };
    expect(place(anchor, tip, "bottom", viewport)).toEqual({ top: 132, left: 320 });
  });

  it("flips to the other side rather than hanging off the edge", () => {
    const anchor = { top: 760, left: 400, width: 40, height: 24 };
    expect(place(anchor, tip, "bottom", viewport).top).toBe(712);

    const rail = { top: 300, left: 960, width: 32, height: 24 };
    expect(place(rail, tip, "right", viewport).left).toBe(752);
  });

  it("clamps the cross axis so a hint at the edge stays whole", () => {
    const anchor = { top: 100, left: 4, width: 24, height: 24 };
    expect(place(anchor, tip, "bottom", viewport).left).toBe(8);
  });
});

/**
 * Every command id named by a hint has to exist. A typo would silently show no
 * chord at all, which is indistinguishable from "not bound" — the one failure
 * mode this whole change exists to remove.
 */
describe("hint call sites", () => {
  // A namespaced id in a `command` prop or field. Bare words are some other
  // field called `command` — the permission dock's label map, for one.
  const REFERENCE = /\bcommand(?:=|: )"(\w+\.[\w.]+)"/g;

  function sources(dir: string, into: string[] = []): string[] {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        if (entry.name !== "commands") sources(path, into);
        continue;
      }
      if (/\.tsx?$/.test(entry.name) && !entry.name.includes(".test.")) into.push(path);
    }
    return into;
  }

  it("names only commands that are in the registry", () => {
    const known = new Set(COMMANDS.map((command) => command.id));
    const unknown = sources(join(__dirname, "..")).flatMap((path) =>
      [...readFileSync(path, "utf8").matchAll(REFERENCE)]
        .filter(([, id]) => !known.has(id))
        .map(([, id]) => `${path}: ${id}`),
    );
    expect(unknown).toEqual([]);
  });
});
