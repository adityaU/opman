import { expect, gateNvimE2E, nvimTest } from "./fixture";

gateNvimE2E();

nvimTest.describe("search", () => {
  nvimTest("searches forward and reverse with visible cursor movement", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("/", "needle", "Enter");
    await expect.poll(nvim.cursorPosition).toMatchObject({ line: 4 });
    await nvim.typeKeys("n", "N", "?", "needle", "Enter");
    await expect.poll(nvim.cursorPosition).toMatchObject({ line: 4 });
  });

  nvimTest("supports star/hash word search and no-highlight command", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("0", "w", "*", "n", "#");
    await expect(nvim.page.locator(".cm-nvim-cursor")).toBeVisible();
    await nvim.typeKeys(":", "noh", "Enter");
    await expect(nvim.page.locator(".cm-nvim-status-panel")).toContainText("Neovim");
    await expect(nvim.page.locator(".cm-searchMatch")).toHaveCount(0);
  });

  nvimTest("renders incremental search matches as visible CodeMirror decorations", async ({ nvim }) => {
    await nvim.typeKeys("/", "needle");
    await expect(nvim.page.locator(".cm-searchMatch")).toHaveCount(3);
    await nvim.typeKeys("Escape");
  });
});
