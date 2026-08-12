import { expect, gateNvimE2E, nvimTest } from "./fixture";

gateNvimE2E();

nvimTest.describe("key hints and resilience", () => {
  nvimTest("shows which-key continuations for leader and g/z families", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("Space");
    await expect(nvim.page.getByRole("dialog", { name: "Keybinding hints" })).toBeVisible();
    await nvim.typeKeys("Escape", "g");
    await expect(nvim.page.getByRole("dialog", { name: "Keybinding hints" })).toBeVisible();
    await nvim.typeKeys("Escape", "z");
    await expect(nvim.page.getByRole("dialog", { name: "Keybinding hints" })).toBeVisible();
  });

  nvimTest("restores buffer content and cursor after a page reload", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("j", "j", "l");
    await expect.poll(nvim.cursorPosition).toEqual({ line: 2, column: 1 });
    await nvim.page.reload({ waitUntil: "domcontentloaded" });
    await nvim.page.locator(".cm-content").waitFor();
    await nvim.expectBufferText((await nvim.readFileOnDisk("main.txt")));
    await expect.poll(nvim.cursorPosition).toEqual({ line: 2, column: 1 });
  });

  nvimTest("surfaces a clean error state after Neovim dies", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.killNvim();
    await expect(nvim.page.locator(".cm-nvim-connection-label")).toHaveText("Neovim unavailable");
    await expect(nvim.page.locator(".cm-nvim-message-label")).toContainText("Neovim exited");
  });

  nvimTest("resyncs after an edit with a stale changedtick", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.sendRawEdit({
      type: "edit",
      changedtick: 0,
      start: { line: 0, column: 0 },
      end: { line: 0, column: 0 },
      lines: ["this edit must be rejected"],
      edit_id: "stale-e2e-edit",
    });
    await nvim.expectBufferText("alpha bravo charlie\none two three four five\nfunction greet(name) { return `Hello ${name}`; }\nbrackets (square [curly {nested}])\nneedle needle needle\nlast line\n");
    await expect.poll(() => nvim.readFileOnDisk("main.txt")).toBe("alpha bravo charlie\none two three four five\nfunction greet(name) { return `Hello ${name}`; }\nbrackets (square [curly {nested}])\nneedle needle needle\nlast line\n");
  });
});
