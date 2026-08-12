/// <reference types="node" />

import { basename } from "node:path";
import { expect, gateNvimE2E, nvimTest } from "./fixture";

gateNvimE2E();

nvimTest.describe("files and buffers", () => {
  nvimTest("opens files from the explorer and switches buffers", async ({ nvim }) => {
    await nvim.seedMain();
    await expect(nvim.page.locator(".xpl-entry-file[title=\"main.txt\"]")).toBeVisible();
    await nvim.openFile("second.txt");
    await expect(nvim.page.locator(".xpl-open-item.is-active[title=\"second.txt\"]")).toContainText("second.txt");
    await nvim.page.locator(".xpl-open-btn").filter({ hasText: "main.txt" }).click();
    await expect(nvim.page.locator(".code-editor-filename")).toHaveText("main.txt");
    await nvim.expectBufferText((await nvim.readFileOnDisk("main.txt")));
  });

  nvimTest("creates and opens a new file", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.page.getByRole("button", { name: "Explorer actions" }).click();
    await nvim.page.getByRole("menuitem", { name: "New file" }).click();
    await nvim.page.locator(".xpl-namefield-input").fill("created.txt");
    await nvim.page.locator(".xpl-namefield-input").press("Enter");
    await expect(nvim.page.locator("button.xpl-entry-file[title=\"created.txt\"]")).toBeVisible();
    await nvim.openFile("created.txt");
    await nvim.expectBufferText("");
    await expect(nvim.page.locator(".xpl-open-item.is-active")).toContainText("created.txt");
  });

  nvimTest("preserves CRLF and multibyte file content", async ({ nvim }) => {
    await nvim.seedBuffer("crlf.txt", "first\r\nsecond\r\nthird\r\n");
    expect(await nvim.readFileOnDisk("crlf.txt")).toContain("first\r\nsecond");
    await nvim.expectBufferText("first\nsecond\nthird");
    await nvim.seedBuffer("unicode.txt", "hello 😀 世界\nemoji column risk\n");
    await nvim.expectBufferText("hello 😀 世界\nemoji column risk");
  });

  nvimTest("opens a 5k-line file without a loading or connection failure", async ({ nvim }) => {
    await nvim.seedBuffer("large.txt", Array.from({ length: 5500 }, (_, index) => `line ${index + 1}`).join("\n") + "\n");
    const started = Date.now();
    await nvim.openFile("large.txt");
    await expect(nvim.page.locator(".cm-line").first()).toContainText("line 1");
    expect(Date.now() - started).toBeLessThan(8_000);
    await expect(nvim.page.locator(".cm-nvim-connection-label")).toHaveText("Neovim");
  });

  nvimTest("uses stable visible file names for open-buffer state", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.openFile("second.txt");
    const labels = await nvim.page.locator(".xpl-open-item .xpl-name").allTextContents();
    expect(labels).toEqual(expect.arrayContaining([basename("main.txt"), basename("second.txt")]));
  });
});
