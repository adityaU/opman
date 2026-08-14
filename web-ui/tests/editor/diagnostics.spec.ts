import { editorTest, expect, gateEditorE2E } from "./fixture";

gateEditorE2E();

const ERROR_FILE = "const answer: string = 42;\nfunction useAnswer() { return answer; }\nexport {};\n";

editorTest.describe("diagnostics", () => {
  editorTest("underlines a type error and counts it in the panel", async ({ editor }) => {
    await editor.openFile("error.ts");
    await editor.expectBufferText(ERROR_FILE);

    // `linter` and `lintGutter` are composed unconditionally by the LSP setup.
    await expect(editor.page.locator(".cm-content .cm-lintRange-error").first())
      .toBeVisible({ timeout: 30_000 });
    await expect(editor.page.locator(".cm-gutter-lint .cm-lint-marker-error").first()).toBeVisible();

    const panel = editor.page.locator(".diagp");
    await expect(panel).not.toHaveClass(/is-clean/);
    await expect(panel.locator(".diagp-count.is-error")).toBeVisible();
  });

  editorTest("lists the diagnostic and jumps to its line", async ({ editor }) => {
    await editor.openFile("error.ts");
    await expect(editor.page.locator(".cm-content .cm-lintRange-error").first())
      .toBeVisible({ timeout: 30_000 });

    await editor.page.locator(".diagp-head").click();
    const item = editor.page.locator(".diagp-item.is-error").first();
    await expect(item).toBeVisible();
    await expect(item.locator(".diagp-item-msg")).toContainText(/string|number|assignable/i);

    await editor.setCursor({ line: 2, column: 0 });
    await item.click();
    await expect.poll(async () => (await editor.cursorPosition()).line).toBe(0);
  });

  editorTest("clears the diagnostic once the type error is fixed", async ({ editor }) => {
    await editor.openFile("error.ts");
    await expect(editor.page.locator(".cm-content .cm-lintRange-error").first())
      .toBeVisible({ timeout: 30_000 });

    // `export {}` must survive, or the file stops being a module and its
    // globals collide with the other fixture file's.
    await editor.setCursor({ line: 0, column: 20 });
    await editor.type("Backspace", "Backspace", "Backspace", "Backspace", "Backspace", "Backspace");
    await editor.typeText("number");
    await editor.expectBufferText(
      "const answer: number = 42;\nfunction useAnswer() { return answer; }\nexport {};\n",
    );

    await expect(editor.page.locator(".cm-content .cm-lintRange-error"))
      .toHaveCount(0, { timeout: 30_000 });
    // The panel may still carry lint hints about the rest of the file; what
    // must go is the error count.
    await expect(editor.page.locator(".diagp .diagp-count.is-error"))
      .toHaveCount(0, { timeout: 30_000 });
  });
});
