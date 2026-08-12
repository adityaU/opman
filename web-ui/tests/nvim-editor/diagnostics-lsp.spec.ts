import { expect, gateNvimE2E, nvimTest } from "./fixture";

gateNvimE2E();

nvimTest.describe("diagnostics and LSP", () => {
  nvimTest("shows diagnostics in a gutter, list, and hover message", async ({ nvim }) => {
    await nvim.openFile("error.ts");
    await expect(nvim.page.locator(".cm-lintRange").first()).toBeVisible({ timeout: 30_000 });
    await expect(nvim.page.locator(".cm-lint-marker-error").first()).toBeVisible();
    await nvim.page.locator(".diagp-head").click();
    await expect(nvim.page.locator(".diagp-list")).toContainText("string");
    await nvim.page.locator(".cm-lintRange").first().hover();
    await expect(nvim.page.locator(".cm-tooltip")).toContainText("string");
  });

  nvimTest("navigates definitions and references and renames a symbol", async ({ nvim }) => {
    await nvim.openFile("definition.ts");
    await nvim.typeKeys("j", "0", "f", "a");
    await nvim.typeKeys("F12");
    await expect(nvim.page.locator(".code-editor-filename")).toHaveText("definition.ts");
    await expect.poll(nvim.cursorPosition).toMatchObject({ line: 0 });
    await nvim.typeKeys("Shift+F12");
    await expect(nvim.page.getByRole("listbox", { name: "References" })).toBeVisible();
    await expect(nvim.page.locator(".cm-lsp-location")).not.toHaveCount(0);
    await nvim.typeKeys("Escape", "0", "f", "a");
    await nvim.typeKeys("F2");
    const field = nvim.page.getByRole("textbox", { name: "New symbol name" });
    await expect(field).toBeVisible();
    await field.fill("renamed");
    await field.press("Enter");
    await expect.poll(() => nvim.readFileOnDisk("definition.ts")).toContain("const renamed = 42");
  });

  nvimTest("shows LSP and native Ctrl-N completion popups", async ({ nvim }) => {
    await nvim.openFile("definition.ts");
    await nvim.typeKeys("j", "0", "i");
    await nvim.expectMode("insert");
    await nvim.typeKeys("Control+n");
    await expect(nvim.page.locator(".cm-tooltip-autocomplete")).toBeVisible({ timeout: 15_000 });
    await nvim.typeKeys("Escape");
  });
});
