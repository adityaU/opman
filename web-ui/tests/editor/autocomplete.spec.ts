import { editorTest, expect, gateEditorE2E } from "./fixture";

gateEditorE2E();

editorTest.describe("autocomplete", () => {
  editorTest("offers language-server completions while typing", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await editor.setCursor({ line: 1, column: 20 });
    await editor.type("Enter");
    await editor.typeText("ans");

    const popup = editor.page.locator(".cm-tooltip-autocomplete");
    await expect(popup).toBeVisible({ timeout: 30_000 });
    await expect(popup.locator("li", { hasText: "answer" }).first()).toBeVisible();
  });

  editorTest("accepts a completion with Enter", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await editor.setCursor({ line: 1, column: 20 });
    await editor.type("Enter");
    await editor.typeText("ans");

    const popup = editor.page.locator(".cm-tooltip-autocomplete");
    await expect(popup).toBeVisible({ timeout: 30_000 });
    await expect(popup.locator("li[aria-selected='true']")).toBeVisible();
    await editor.type("Enter");

    await expect(popup).toBeHidden();
    await expect.poll(async () => (await editor.bufferText()).split("\n")[2]).toBe("answer");
  });

  editorTest("narrows the list as more characters are typed", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await editor.setCursor({ line: 1, column: 20 });
    await editor.type("Enter");
    await editor.typeText("a");

    const options = editor.page.locator(".cm-tooltip-autocomplete li");
    await expect(editor.page.locator(".cm-tooltip-autocomplete")).toBeVisible({ timeout: 30_000 });
    const wide = await options.count();

    await editor.typeText("nswe");
    await expect.poll(() => options.count()).toBeLessThanOrEqual(wide);
    await expect(options.first()).toContainText("answer");
  });

  editorTest("dismisses the popup with Escape and leaves the text alone", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await editor.setCursor({ line: 1, column: 20 });
    await editor.type("Enter");
    await editor.typeText("ans");

    const popup = editor.page.locator(".cm-tooltip-autocomplete");
    await expect(popup).toBeVisible({ timeout: 30_000 });
    await editor.type("Escape");
    await expect(popup).toBeHidden();
    await expect.poll(async () => (await editor.bufferText()).split("\n")[2]).toBe("ans");
  });
});
