/**
 * Slash commands popover tests.
 *
 * Note: Leptos has 27 built-in commands hardcoded in the WASM binary
 * (slash_command_popover.rs), so we cannot override them via API mocks.
 * Tests are written to work with the real built-in command set.
 */
import { test, expect } from "@playwright/test";
import { setupMockAPI } from "./helpers";

async function setupSlashTests(page: import("@playwright/test").Page) {
  await setupMockAPI(page);
  await page.goto("/ui");
  await page.waitForSelector(".chat-layout", { timeout: 15_000 });
}

test.describe("Slash command popover", () => {
  test.beforeEach(async ({ page }) => {
    await setupSlashTests(page);
  });

  test("appears when typing / at start of input", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await textarea.focus();
    await textarea.fill("/");

    await expect(page.locator(".slash-popover")).toBeVisible({ timeout: 3_000 });
  });

  test("shows all commands", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await textarea.focus();
    await textarea.fill("/");

    await expect(page.locator(".slash-popover")).toBeVisible({ timeout: 3_000 });
    const items = page.locator(".slash-popover-item");
    const count = await items.count();
    // Leptos has 27 built-in commands; count may grow if API adds more
    expect(count).toBeGreaterThanOrEqual(20);
  });

  test("filters commands as user types", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await textarea.focus();
    await textarea.fill("/compact");

    await expect(page.locator(".slash-popover")).toBeVisible({ timeout: 3_000 });
    const items = page.locator(".slash-popover-item");
    const count = await items.count();
    expect(count).toBe(1);
    await expect(items.first().locator(".slash-popover-name")).toContainText("compact");
  });

  test("hides popover when no commands match", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await textarea.focus();
    await textarea.fill("/zzzznonexistent");

    // Popover should either be hidden or show 0 items
    const popover = page.locator(".slash-popover");
    const isVisible = await popover.isVisible().catch(() => false);
    if (isVisible) {
      await expect(popover.locator(".slash-popover-item")).toHaveCount(0);
    }
  });

  test("does not appear when / is not at start", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await textarea.focus();
    await textarea.fill("hello /");

    await page.waitForTimeout(500);
    await expect(page.locator(".slash-popover")).not.toBeVisible();
  });

  test("clicking a command inserts it", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await textarea.focus();
    await textarea.fill("/");

    await expect(page.locator(".slash-popover")).toBeVisible({ timeout: 3_000 });

    // Click the "compact" command (unique name, no ambiguity)
    const compactItem = page.locator(".slash-popover-item").filter({
      has: page.locator(".slash-popover-name", { hasText: "/compact" }),
    });
    await compactItem.click();

    const value = await textarea.inputValue();
    expect(value).toContain("compact");
  });

  test("Escape closes the popover", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await textarea.focus();
    await textarea.fill("/");

    await expect(page.locator(".slash-popover")).toBeVisible({ timeout: 3_000 });

    await page.keyboard.press("Escape");
    await expect(page.locator(".slash-popover")).not.toBeVisible();
  });
});
