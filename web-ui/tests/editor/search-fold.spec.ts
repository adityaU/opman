import { editorTest, expect, gateEditorE2E, mainFixtureText } from "./fixture";

gateEditorE2E();

editorTest.describe("search and folding", () => {
  editorTest("finds matches through the CodeMirror search panel", async ({ editor }) => {
    // `searchKeymap` was one of the keymaps the Neovim gate removed.
    await editor.type("ControlOrMeta+f");
    const panel = editor.page.locator(".cm-panel.cm-search");
    await expect(panel).toBeVisible();

    const field = panel.locator("input[name='search']");
    await field.fill("needle");
    await panel.getByRole("button", { name: /^next$/i }).click();
    // Line 4 is the only line holding `needle`.
    await expect.poll(async () => (await editor.cursorPosition()).line).toBe(4);

    await field.press("Escape");
    await expect(panel).toBeHidden();
  });

  editorTest("replaces a match from the search panel", async ({ editor }) => {
    await editor.type("ControlOrMeta+f");
    const panel = editor.page.locator(".cm-panel.cm-search");
    await expect(panel).toBeVisible();

    await panel.locator("input[name='search']").fill("needle");
    await panel.locator("input[name='replace']").fill("thread");
    await panel.getByRole("button", { name: /replace all/i }).click();

    await editor.expectBufferText(mainFixtureText.replaceAll("needle", "thread"));
  });

  editorTest("folds and unfolds a block from the gutter", async ({ editor }) => {
    await editor.openFile("fold.txt");
    await editor.expectBufferText("function outer() {\n  const value = 42;\n  return value;\n}\n");

    // Native `foldGutter()` replaced the Neovim fold-sync gutter. Its marker is
    // an SVG chevron, so it is matched by class rather than by text.
    // The clickable target is the gutter row, which is what a user aims at; the
    // chevron inside it is a decoration.
    // CodeMirror keeps a hidden sizing row in the gutter that also carries a
    // marker, so the visible one is the one a user can aim at.
    const marker = editor.page
      .locator(".cm-foldGutter .cm-gutterElement")
      .filter({ has: editor.page.locator(".cm-opman-fold-marker") })
      .filter({ visible: true })
      .first();
    await expect(marker).toBeVisible();
    await marker.click();
    await expect(editor.page.locator(".cm-foldPlaceholder")).toBeVisible();

    await editor.type("Control+Shift+BracketRight");
    await expect(editor.page.locator(".cm-foldPlaceholder")).toHaveCount(0, { timeout: 15_000 });
  });

  editorTest("folds through the fold keymap without touching the document", async ({ editor }) => {
    await editor.openFile("fold.txt");
    const original = await editor.bufferText();
    await editor.setCursor({ line: 0, column: 0 });

    // `foldKeymap` binds Ctrl-Shift-[ to fold the block at the cursor.
    await editor.type("Control+Shift+BracketLeft");
    await expect(editor.page.locator(".cm-foldPlaceholder")).toBeVisible();
    // Folding is a view concern: the document must be unchanged.
    await editor.expectBufferText(original);

    await editor.type("Control+Shift+BracketRight");
    await expect(editor.page.locator(".cm-foldPlaceholder")).toHaveCount(0);
  });
});
