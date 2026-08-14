import { editorTest, expect, gateEditorE2E } from "./fixture";

gateEditorE2E();

editorTest.describe("language server features", () => {
  // Hover itself is covered by `hover-card.spec.ts`, which exercises the card's
  // signature, actions, theming, jumps and hold. A second hover assertion here
  // duplicated that and was fragile about where in the run it landed.
  editorTest("jumps to a definition with F12", async ({ editor }) => {
    await editor.openFile("definition.ts");
    // `answer` on the console.log line resolves to its declaration on line 0.
    await editor.setCursor({ line: 1, column: 13 });
    await editor.type("F12");
    await expect.poll(async () => (await editor.cursorPosition()).line, { timeout: 30_000 }).toBe(0);
  });

  editorTest("lists references in the LSP panel", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await editor.setCursor({ line: 0, column: 6 });
    await editor.type("Shift+F12");

    const panel = editor.page.locator(".cm-lsp-panel");
    await expect(panel).toBeVisible({ timeout: 30_000 });
    await expect(panel.locator(".cm-lsp-location").first()).toBeVisible({ timeout: 30_000 });

    await editor.page.locator(".cm-content").press("Escape");
    await expect(panel).toBeHidden();
  });

  editorTest("renames a symbol across the file with F2", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await editor.setCursor({ line: 0, column: 6 });
    await editor.type("F2");

    const input = editor.page.locator("#cm-lsp-rename-input");
    await expect(input).toBeVisible({ timeout: 30_000 });
    await input.fill("resolved");
    await input.press("Enter");

    await expect.poll(editor.bufferText, { timeout: 30_000 })
      .toBe("const resolved = 42;\nconsole.log(resolved);\nexport {};\n");
  });
});
