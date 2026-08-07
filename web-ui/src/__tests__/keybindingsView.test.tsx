import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_CONFIG } from "../keybindings/config";
import { KeymapProvider } from "../keybindings/KeymapContext";
import { KeybindingsModal } from "../keybindings-view/KeybindingsModal";
import { buildRows, filterRows } from "../keybindings-view/rows";
import { builtInLayers } from "../keybindings/layers";
import { Keymap } from "../keybindings/matcher";
import { resolve } from "../keybindings/resolve";
import type { Host } from "../keybindings/types";

const HOST: Host = { platform: "linux", target: "web", browser: "chrome" };

const saved: unknown[] = [];

vi.mock("../api/keybindings", () => ({
  loadKeybindingsOrDefault: vi.fn(async () => ({
    config: { ...DEFAULT_CONFIG },
    diagnostics: [],
    path: "/home/u/.config/opman/keybindings.json",
  })),
  saveKeybindings: vi.fn(async (config: unknown) => {
    saved.push(config);
    return { config, diagnostics: [], path: "/home/u/.config/opman/keybindings.json" };
  }),
}));

function keymap(mode: "normal" | "vim" = "normal") {
  return new Keymap(resolve(builtInLayers(), { host: HOST, mode }).bindings);
}

function mount(mode: "normal" | "vim" = "normal") {
  return render(
    <KeymapProvider config={{ ...DEFAULT_CONFIG, mode }} host={HOST}>
      <KeybindingsModal onClose={() => undefined} />
    </KeymapProvider>,
  );
}

beforeEach(() => {
  saved.length = 0;
});

describe("rows", () => {
  it("lists a row per bound chord and a row for every unbound command", () => {
    const rows = buildRows(keymap(), HOST);
    const sidebar = rows.filter((r) => r.command.id === "layout.toggleSidebar");
    expect(sidebar).toHaveLength(1);
    expect(sidebar[0].chord).toBe("Ctrl+B");

    const unbound = rows.filter((r) => r.source === "unbound");
    expect(unbound.length).toBeGreaterThan(0);
    expect(unbound.every((r) => r.chord === "")).toBe(true);
  });

  it("shows both chords of a command bound twice", () => {
    const rows = buildRows(keymap(), HOST);
    const palette = rows.filter((r) => r.command.id === "palette.commands").map((r) => r.chord);
    expect(palette).toEqual(expect.arrayContaining(["Ctrl+Shift+P", "f1"]));
  });

  it("filters by title, id, category and chord", () => {
    const rows = buildRows(keymap(), HOST);
    expect(filterRows(rows, { query: "toggle sidebar" })).toHaveLength(1);
    expect(filterRows(rows, { query: "layout.toggleSidebar" })).toHaveLength(1);
    expect(filterRows(rows, { query: "Ctrl+B" }).length).toBeGreaterThan(0);
    expect(filterRows(rows, { query: "zzzz" })).toHaveLength(0);
  });

  // Recording answers "what does this key do", which for a chord shared across
  // scopes is more than one thing — that is the answer, not a bug.
  it("answers what a recorded chord is bound to, in every scope", () => {
    const rows = buildRows(keymap(), HOST);
    const found = filterRows(rows, { query: "", chordId: "ctrl+b" });
    expect(found.map((r) => r.command.id)).toEqual(["layout.toggleSidebar", "doc.bold"]);
  });

  it("narrows to unbound commands", () => {
    const rows = buildRows(keymap(), HOST);
    const unbound = filterRows(rows, { query: "", onlyUnbound: true });
    expect(unbound.every((r) => r.source === "unbound")).toBe(true);
  });
});

describe("KeybindingsModal", () => {
  it("renders the table with commands and their chords", async () => {
    mount();
    expect(await screen.findByText("Toggle Sidebar")).toBeTruthy();
    expect(screen.getAllByText("Ctrl+B").length).toBeGreaterThan(0);
  });

  it("searches by command title", async () => {
    mount();
    await screen.findByText("Toggle Sidebar");
    await userEvent.type(screen.getByPlaceholderText("Search commands and keys"), "compact");

    expect(screen.getByText("Compact History")).toBeTruthy();
    expect(screen.queryByText("Toggle Sidebar")).toBeNull();
  });

  it("shows the leader tree only in vim mode", async () => {
    const { unmount } = mount("normal");
    await screen.findByText("Toggle Sidebar");
    expect(screen.queryByText("Leader tree")).toBeNull();
    unmount();

    mount("vim");
    // In vim mode the command has two chords — Ctrl+B and <leader>wb — so the
    // title appears once per row.
    await screen.findAllByText("Toggle Sidebar");
    fireEvent.click(screen.getByText("Leader tree"));
    expect(screen.getByText("+git")).toBeTruthy();
    expect(screen.getAllByText(/Free:/).length).toBeGreaterThan(0);
  });

  it("writes a mode switch to the config", async () => {
    mount();
    await screen.findByText("Toggle Sidebar");
    // getByRole with a name filter computes accessible names across the whole
    // table, which is seconds on a 200-row list. Query the control directly.
    fireEvent.click(screen.getByText("Vim"));

    expect(saved).toHaveLength(1);
    expect(saved[0]).toMatchObject({ mode: "vim" });
  });

  it("removes a binding and records the removal entry", async () => {
    mount();
    const title = await screen.findByText("Toggle Sidebar");
    const row = title.closest("li");
    expect(row).toBeTruthy();

    await userEvent.click(within(row as HTMLElement).getByTitle("Remove this keybinding"));
    expect(saved[0]).toMatchObject({
      bindings: [{ key: "ctrl+b", command: "-layout.toggleSidebar" }],
    });
  });
});

describe("capture dialog", () => {
  it("records a chord and saves it against the command", async () => {
    mount();
    const title = await screen.findByText("Toggle Sidebar");
    await userEvent.click(title);

    const dialog = await screen.findByRole("dialog", { name: "Record keybinding" });
    await userEvent.keyboard("{Control>}{Alt>}j{/Alt}{/Control}");
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }));

    expect(saved[0]).toMatchObject({
      bindings: [
        { key: "ctrl+b", command: "-layout.toggleSidebar" },
        { key: "ctrl+alt+j", command: "layout.toggleSidebar" },
      ],
    });
  });

  it("warns and blocks on a chord the browser reserves", async () => {
    mount();
    await userEvent.click(await screen.findByText("Toggle Sidebar"));
    const dialog = await screen.findByRole("dialog", { name: "Record keybinding" });

    await userEvent.keyboard("{Control>}w{/Control}");
    expect(within(dialog).getByText(/close browser tab/)).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "Unusable chord" })).toBeDisabled();
  });

  it("warns when the chord is already the prefix of a longer one", async () => {
    mount();
    await userEvent.click(await screen.findByText("Toggle Sidebar"));
    const dialog = await screen.findByRole("dialog", { name: "Record keybinding" });

    await userEvent.keyboard("{Control>}k{/Control}");
    expect(within(dialog).getByText(/cannot also run a command/)).toBeTruthy();
  });

  it("warns about a same-scope conflict without blocking the save", async () => {
    mount();
    await userEvent.click(await screen.findByText("Session Watcher"));
    const dialog = await screen.findByRole("dialog", { name: "Record keybinding" });

    await userEvent.keyboard("{Control>}b{/Control}");
    expect(within(dialog).getByText(/Already bound to layout.toggleSidebar/)).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "Save" })).not.toBeDisabled();
  });
});
