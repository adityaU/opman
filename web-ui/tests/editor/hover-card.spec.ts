import { editorTest, expect, gateEditorE2E } from "./fixture";
import type { Page } from "@playwright/test";

gateEditorE2E();

/** Park the pointer on `answer` in `console.log(answer)` and wait for the card. */
async function hoverSymbol(page: Page): Promise<void> {
  await expect.poll(() => page.locator(".cm-content[data-lsp='on']").count(), {
    timeout: 30_000,
  }).toBeGreaterThan(0);
  await page.waitForTimeout(5_000);
  const point = await page.evaluate(() => {
    const node = document.querySelector(".cm-content") as unknown as {
      cmTile?: { root?: { view?: {
        state: { doc: { line(n: number): { from: number } } };
        coordsAtPos(pos: number): { left: number; right: number; top: number; bottom: number } | null;
      } } };
    };
    const view = node?.cmTile?.root?.view;
    const box = view?.coordsAtPos(view.state.doc.line(2).from + 13);
    return box ? { x: (box.left + box.right) / 2, y: (box.top + box.bottom) / 2 } : null;
  });
  if (!point) throw new Error("could not locate the symbol on screen");
  await page.mouse.move(point.x - 6, point.y);
  await page.mouse.move(point.x, point.y);
  await expect(page.locator(".lsph-card")).toBeVisible({ timeout: 20_000 });
}

editorTest.describe("LSP hover card", () => {
  editorTest("shows the signature and the actions the server supports", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await hoverSymbol(editor.page);

    const card = editor.page.locator(".lsph-card");
    await expect(card.locator(".lsph-signature")).toContainText("answer");
    // Driven by the server's own capabilities, not a hardcoded list.
    await expect(card.getByRole("button", { name: "Definition", exact: true })).toBeVisible();
    await expect(card.getByRole("button", { name: "References", exact: true })).toBeVisible();
    await expect(card.getByRole("button", { name: "Rename", exact: true })).toBeVisible();
  });

  editorTest("wears the app's popover surface in both themes", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await hoverSymbol(editor.page);

    const card = editor.page.locator(".lsph-card");
    await expect(card).toHaveClass(/modal-popover-surface/);

    const paint = async () => card.evaluate((node) => {
      const style = getComputedStyle(node);
      return { background: style.backgroundColor, border: style.borderTopColor };
    });
    // Probed inside the card's own parent: the editor subtree redefines the
    // elevation tokens, so reading `--modal-surface` off <html> answers for a
    // different cascade than the one the card is painted in.
    const tokenSurface = async () => card.evaluate((node) => {
      const probe = document.createElement("span");
      probe.style.color = "var(--modal-surface)";
      node.parentElement?.append(probe);
      const resolved = getComputedStyle(probe).color;
      probe.remove();
      return resolved;
    });

    const asRgb = (value: string) => value.replace(/\s+/g, "");
    expect(asRgb((await paint()).background)).toBe(asRgb(await tokenSurface()));

    await editor.page.evaluate(() => {
      document.documentElement.classList.add("flat-theme");
      localStorage.setItem("opman-theme-mode", "flat");
    });
    // Still the token, whatever the flat theme sets it to — never a literal.
    await expect
      .poll(async () => asRgb((await paint()).background))
      .toBe(asRgb(await tokenSurface()));
  });

  editorTest("jumps to the definition from the card", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await editor.setCursor({ line: 1, column: 0 });
    await hoverSymbol(editor.page);

    await editor.page.locator(".lsph-card").getByRole("button", { name: "Definition", exact: true }).click();
    // `answer` is declared on line 0 of the same file.
    await expect.poll(async () => (await editor.cursorPosition()).line, { timeout: 20_000 }).toBe(0);
  });

  editorTest("offers a pane for the jump instead of taking one", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await hoverSymbol(editor.page);

    const card = editor.page.locator(".lsph-card");
    const where = card.getByRole("button", { name: "Definition in another pane" });
    await expect(where).toBeVisible();
    await where.click();

    // With a second pane on screen the workspace asks which one; with only the
    // single e2e pane it answers itself, and the file simply opens.
    const overlay = editor.page.locator(".wsp-target");
    const asked = await overlay.isVisible().catch(() => false);
    if (asked) {
      await expect(overlay.locator(".wsp-target-chip-label")).toContainText("Definition");
      await expect(overlay.locator(".wsp-target-chip-keys")).toContainText("vertical");
      await expect(overlay.locator(".wsp-target-chip-keys")).toContainText("horizontal");
      await expect(overlay.locator(".wsp-target-chip-keys")).toContainText("window");
      await editor.page.keyboard.press("Escape");
      await expect(overlay).toBeHidden();
      return;
    }
    await expect(editor.page.locator(".code-editor-filename")).toHaveText("definition.ts");
  });

  editorTest("opens the references list from the card", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await hoverSymbol(editor.page);

    await editor.page.locator(".lsph-card").getByRole("button", { name: "References", exact: true }).click();
    const panel = editor.page.locator(".cm-lsp-panel");
    await expect(panel).toBeVisible({ timeout: 30_000 });
    await expect(panel.locator(".cm-lsp-location").first()).toBeVisible({ timeout: 30_000 });
  });

  editorTest("starts a rename from the card", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await hoverSymbol(editor.page);

    await editor.page.locator(".lsph-card").getByRole("button", { name: "Rename", exact: true }).click();
    await expect(editor.page.locator("#cm-lsp-rename-input")).toBeVisible({ timeout: 30_000 });
  });
});

editorTest.describe("hover card appearance", () => {
  editorTest("captures the card in both themes", async ({ editor }) => {
    await editor.openFile("definition.ts");
    await hoverSymbol(editor.page);
    await editor.page.screenshot({ path: "tests/screenshots/hover-card-glassy.png" });
    await editor.page.evaluate(() => document.documentElement.classList.add("flat-theme"));
    await editor.page.waitForTimeout(300);
    await editor.page.screenshot({ path: "tests/screenshots/hover-card-flat.png" });
  });
});

/** Hover a named identifier on a given line of the open file. */
async function hoverAt(page: Page, line: number, symbol: string): Promise<void> {
  await expect.poll(() => page.locator(".cm-content[data-lsp='on']").count(), {
    timeout: 30_000,
  }).toBeGreaterThan(0);
  await page.waitForTimeout(5_000);
  const point = await page.evaluate(({ line: l, symbol }) => {
    const node = document.querySelector(".cm-content") as unknown as {
      cmTile?: { root?: { view?: {
        state: { doc: { line(n: number): { from: number; text: string } } };
        coordsAtPos(pos: number): { left: number; right: number; top: number; bottom: number } | null;
      } } };
    };
    const view = node?.cmTile?.root?.view;
    if (!view) return null;
    const row = view.state.doc.line(l + 1);
    const at = row.text.indexOf(symbol);
    if (at < 0) return null;
    // Aim at the middle of the identifier rather than at a computed column,
    // which lands on a space the moment the line changes shape.
    const box = view.coordsAtPos(row.from + at + Math.floor(symbol.length / 2));
    return box ? { x: (box.left + box.right) / 2, y: (box.top + box.bottom) / 2 } : null;
  }, { line, symbol });
  if (!point) throw new Error("could not locate the symbol on screen");
  await page.mouse.move(point.x - 6, point.y);
  await page.mouse.move(point.x, point.y);
  await expect(page.locator(".lsph-card")).toBeVisible({ timeout: 20_000 });
}

editorTest.describe("hover card navigation across files", () => {
  editorTest("renames the header when the jump lands in another file", async ({ editor }) => {
    await editor.openFile("consumer.ts");
    // `shared` on line 1 is declared in lib.ts.
    await hoverAt(editor.page, 1, "shared");

    await editor.page.locator(".lsph-card").getByRole("button", { name: "Definition", exact: true }).click();
    // The header used to keep the old name: the absolute path the server
    // returned was folded onto whichever file was already open.
    await expect(editor.page.locator(".code-editor-filename")).toHaveText("lib.ts", { timeout: 20_000 });
    await expect.poll(editor.bufferText).toContain("export const shared");
  });

  editorTest("holds the card open after the pointer leaves", async ({ editor }) => {
    await editor.openFile("consumer.ts");
    await hoverAt(editor.page, 1, "shared");

    const card = editor.page.locator(".lsph-card");
    // Well clear of the symbol, and of the card itself.
    await editor.page.mouse.move(20, 400);
    await editor.page.waitForTimeout(2_000);
    await expect(card).toBeVisible();
  });

  editorTest("replaces the card when a different symbol is hovered", async ({ editor }) => {
    await editor.openFile("consumer.ts");
    await hoverAt(editor.page, 1, "shared");
    await expect(editor.page.locator(".lsph-card")).toContainText("shared");

    await hoverAt(editor.page, 1, "box");
    await expect.poll(
      () => editor.page.locator(".lsph-card").first().textContent(),
      { timeout: 20_000 },
    ).toContain("box");
  });
});
