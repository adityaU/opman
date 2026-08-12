import { expect, gateNvimE2E, nvimTest } from "./fixture";

gateNvimE2E();

nvimTest.describe("windows, tabs, folds and lists", () => {
  nvimTest("renders Neovim splits and Ctrl-W navigation", async ({ nvim }) => {
    await nvim.typeKeys(":", "sp", "Enter", ":", "vs", "Enter", "Control+w", "h");
    await expect(nvim.page.locator(".nvim-tabpage-group")).toBeVisible();
    await expect(nvim.page.locator(".nvim-tabpage-windows")).toContainText("3 windows");
  });

  nvimTest("renders tab pages and modified buffer tab indicators", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("x");
    await nvim.typeKeys(":", "tabnew", "Enter");
    await expect(nvim.page.getByRole("tablist", { name: "Open buffers" })).toBeVisible();
    await expect(nvim.page.locator(".nvim-tabpage-count")).toContainText("2 tabs");
    await expect(nvim.page.locator(".nvim-buffer-tab-modified")).toBeVisible();
  });

  nvimTest("keeps CodeMirror's fold gutter visible", async ({ nvim }) => {
    await nvim.seedBuffer("main.txt", "function outer() {\n  const value = 42;\n  return value;\n}\n");
    await expect(nvim.page.locator(".cm-foldGutter")).toBeVisible();
    await nvim.typeKeys("z", "c");
    await expect(nvim.page.locator(".cm-foldPlaceholder")).toBeVisible();
    await nvim.typeKeys("z", "o");
    await expect(nvim.page.locator(".cm-foldPlaceholder")).toHaveCount(0);
    await nvim.typeKeys("z", "R", "z", "M");
    await expect(nvim.page.locator(".cm-foldPlaceholder")).toHaveCount(1);
  });

  nvimTest("populates and navigates a quickfix or location list", async ({ nvim }) => {
    await nvim.typeKeys(":", "vimgrep /needle/ main.txt", "Enter", ":", "cn", "Enter", ":", "cp", "Enter");
    await expect(nvim.page.locator(".nvim-messages-overlay")).toContainText("needle");
  });
});
