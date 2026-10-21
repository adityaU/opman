import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import { installFeatures, type FeatureHooks } from "../terminal-panel/features";
import { TerminalMenu } from "../terminal-panel/TerminalMenu";
import { FONT_CHANGED_EVENT } from "../terminal-panel/fontSize";

/** The parts of xterm the features touch, recording what they do. */
function fakeTerm(selection = "") {
  let handler: ((event: KeyboardEvent) => boolean) | undefined;
  let osc: ((payload: string) => boolean) | undefined;
  const term = {
    options: { fontSize: 13 },
    rows: 24,
    modes: { mouseTrackingMode: "none" },
    unicode: { activeVersion: "6" },
    buffer: { active: { length: 200, viewportY: 150 } },
    loadAddon: vi.fn(),
    hasSelection: () => selection !== "",
    getSelection: () => selection,
    selectAll: vi.fn(),
    selectLines: vi.fn(),
    clearSelection: vi.fn(),
    scrollToTop: vi.fn(),
    scrollToBottom: vi.fn(),
    scrollToLine: vi.fn(),
    paste: vi.fn(),
    focus: vi.fn(),
    blur: vi.fn(),
    attachCustomKeyEventHandler: (fn: (event: KeyboardEvent) => boolean) => { handler = fn; },
    parser: { registerOscHandler: (_id: number, fn: (payload: string) => boolean) => { osc = fn; return { dispose: vi.fn() }; } },
  };
  return {
    term: term as unknown as Terminal,
    raw: term,
    key: (init: KeyboardEventInit & { key: string }) => {
      const event = new KeyboardEvent("keydown", { cancelable: true, ...init });
      return { event, passedToXterm: handler?.(event) };
    },
    osc: (payload: string) => osc?.(payload),
  };
}

function setup(selection = "", mac = false) {
  Object.defineProperty(navigator, "platform", { value: mac ? "MacIntel" : "Linux x86_64", configurable: true });
  const fake = fakeTerm(selection);
  const container = document.createElement("div");
  document.body.appendChild(container);
  const fit = { fit: vi.fn() } as unknown as FitAddon;
  const hooks: FeatureHooks = { onMenu: vi.fn(), onNotice: vi.fn(), onCopyMode: vi.fn() };
  const installed = installFeatures(fake.term, container, fit, hooks);
  return { ...fake, ...installed, container, fit, hooks };
}

describe("terminal features", () => {
  let writeText: ReturnType<typeof vi.fn>;
  beforeEach(() => {
    writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { platform: "Linux x86_64", clipboard: { writeText, readText: vi.fn().mockResolvedValue("clip") } });
  });
  afterEach(() => { vi.unstubAllGlobals(); document.body.innerHTML = ""; });

  it("turns on wide-character support", () => {
    const t = setup();
    expect(t.raw.unicode.activeVersion).toBe("11");
    expect(t.raw.loadAddon).toHaveBeenCalledTimes(1);
  });

  it("copies the selection on Ctrl+Shift+C and keeps the key from xterm and the page", async () => {
    const t = setup("selected text");
    const { event, passedToXterm } = t.key({ key: "C", ctrlKey: true, shiftKey: true });
    expect(passedToXterm).toBe(false);
    expect(event.defaultPrevented).toBe(true);
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith("selected text"));
    await vi.waitFor(() => expect(t.hooks.onNotice).toHaveBeenCalledWith("Copied"));
  });

  it("sends Ctrl+C to the shell when nothing is selected, and copies when something is", () => {
    const none = setup("");
    const interrupt = none.key({ key: "c", ctrlKey: true });
    expect(interrupt.passedToXterm).toBe(true);
    expect(interrupt.event.defaultPrevented).toBe(false);

    const some = setup("abc");
    expect(some.key({ key: "c", ctrlKey: true }).passedToXterm).toBe(false);
  });

  it("leaves paste to the browser: xterm skips the key but the page still gets it", () => {
    const t = setup();
    const { event, passedToXterm } = t.key({ key: "V", ctrlKey: true, shiftKey: true });
    expect(passedToXterm).toBe(false);
    expect(event.defaultPrevented).toBe(false);
  });

  it("uses Cmd for copy and paste on a Mac", () => {
    const t = setup("abc", true);
    expect(t.key({ key: "c", metaKey: true }).event.defaultPrevented).toBe(true);
    expect(t.key({ key: "v", metaKey: true }).event.defaultPrevented).toBe(false);
  });

  it("selects all, scrolls to the ends, and changes the font", () => {
    const t = setup();
    t.key({ key: "A", ctrlKey: true, shiftKey: true });
    expect(t.raw.selectAll).toHaveBeenCalled();
    t.key({ key: "Home", shiftKey: true });
    t.key({ key: "End", shiftKey: true });
    expect(t.raw.scrollToTop).toHaveBeenCalled();
    expect(t.raw.scrollToBottom).toHaveBeenCalled();

    t.key({ key: "=", ctrlKey: true });
    window.dispatchEvent(new CustomEvent(FONT_CHANGED_EVENT, { detail: 18 }));
    expect(t.raw.options.fontSize).toBe(18);
    expect(t.fit.fit).toHaveBeenCalled();
  });

  it("copies what a program sends by OSC 52, and nothing for a read request", async () => {
    const t = setup();
    expect(t.osc(`c;${btoa("from tmux")}`)).toBe(true);
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith("from tmux"));
    writeText.mockClear();
    expect(t.osc("c;?")).toBe(true);
    expect(writeText).not.toHaveBeenCalled();
  });

  it("shows the menu on right-click, unless a program has the mouse", () => {
    const t = setup();
    fireEvent.contextMenu(t.container, { clientX: 30, clientY: 40 });
    expect(t.hooks.onMenu).toHaveBeenCalledWith(30, 40);

    t.raw.modes.mouseTrackingMode = "any";
    fireEvent.contextMenu(t.container);
    expect(t.hooks.onMenu).toHaveBeenCalledTimes(1);
    fireEvent.contextMenu(t.container, { shiftKey: true });
    expect(t.hooks.onMenu).toHaveBeenCalledTimes(2);
  });

  it("pastes the clipboard on middle-click and dropped text", async () => {
    const t = setup();
    fireEvent(t.container, new MouseEvent("auxclick", { button: 1, bubbles: true, cancelable: true }));
    await vi.waitFor(() => expect(t.raw.paste).toHaveBeenCalledWith("clip"));
    const drop = new Event("drop", { bubbles: true, cancelable: true }) as DragEvent;
    Object.defineProperty(drop, "dataTransfer", { value: { getData: () => "dropped", types: ["text/plain"] } });
    t.container.dispatchEvent(drop);
    expect(t.raw.paste).toHaveBeenCalledWith("dropped");
  });

  it("copy mode: moves, selects lines, copies and leaves", async () => {
    const t = setup("line text");
    t.actions.copyMode();
    expect(t.hooks.onCopyMode).toHaveBeenLastCalledWith(true);
    expect(t.raw.selectLines).toHaveBeenLastCalledWith(173, 173);

    expect(t.key({ key: "k" }).passedToXterm).toBe(false);
    expect(t.raw.selectLines).toHaveBeenLastCalledWith(172, 172);
    t.key({ key: "v" });
    t.key({ key: "k" });
    expect(t.raw.selectLines).toHaveBeenLastCalledWith(171, 172);
    t.key({ key: "y" });
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith("line text"));
    await vi.waitFor(() => expect(t.hooks.onCopyMode).toHaveBeenLastCalledWith(false));
    expect(t.raw.clearSelection).toHaveBeenCalled();
    // Out of copy mode again, keys reach the shell.
    expect(t.key({ key: "k" }).passedToXterm).toBe(true);
  });

  it("stops listening when disposed", () => {
    const t = setup();
    t.dispose();
    fireEvent.contextMenu(t.container);
    expect(t.hooks.onMenu).not.toHaveBeenCalled();
  });
});

describe("TerminalMenu", () => {
  const props = () => ({
    x: 10, y: 10, canCopy: true,
    onCopy: vi.fn(), onPaste: vi.fn(), onSelectAll: vi.fn(), onFind: vi.fn(),
    onCopyMode: vi.fn(), onClear: vi.fn(), onClose: vi.fn(),
  });

  it("lists the actions, runs one, and closes", () => {
    const p = props();
    render(<TerminalMenu {...p} />);
    for (const name of [/^Copy(?! Mode)/, /^Paste/, /^Select All/, /^Find…/, /^Copy Mode/, /^Clear/]) {
      expect(screen.getByRole("menuitem", { name })).toBeTruthy();
    }
    fireEvent.click(screen.getByRole("menuitem", { name: /Paste/ }));
    expect(p.onPaste).toHaveBeenCalled();
    expect(p.onClose).toHaveBeenCalled();
  });

  it("disables Copy with no selection", () => {
    render(<TerminalMenu {...props()} canCopy={false} />);
    expect((screen.getByRole("menuitem", { name: /^Copy(?! Mode)/ }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("closes on Escape and on a click outside", () => {
    const p = props();
    render(<TerminalMenu {...p} />);
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.pointerDown(document.body);
    expect(p.onClose).toHaveBeenCalledTimes(2);
  });
});
