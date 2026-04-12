/**
 * Playwright tests for the Slash Command Popover.
 *
 * Covers:
 *  - Popover appears when typing `/` in an empty prompt
 *  - Popover shows built-in commands
 *  - Filter narrows the command list
 *  - Keyboard navigation (ArrowDown, ArrowUp)
 *  - Selecting a command with Enter
 *  - Selecting a command with Tab
 *  - Selecting a command with click
 *  - No-arg commands clear the textarea
 *  - Commands with args leave `/<command> ` in textarea
 *  - Escape closes the popover
 *  - Popover disappears when text no longer starts with `/`
 *  - Popover hides when filter matches nothing
 */

import { test, expect, Page } from "@playwright/test";
import { setupMockAPI, navigateAuthenticated } from "./helpers";

// ── Helper: more slash commands for richer filtering tests ──
const EXTENDED_COMMANDS = [
  { name: "new", description: "Start a new session" },
  { name: "model", description: "Switch model", args: "<model-name>" },
  { name: "compact", description: "Compact conversation history" },
  { name: "undo", description: "Undo last action" },
  { name: "redo", description: "Redo last action" },
  { name: "share", description: "Share session" },
  { name: "theme", description: "Change color theme", args: "<theme>" },
  { name: "agent", description: "Switch agent type", args: "<agent>" },
];

async function setupWithExtendedCommands(page: Page) {
  await setupMockAPI(page);
  // Override /api/commands with extended set (last registered = highest priority)
  await page.route("**/api/commands", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(EXTENDED_COMMANDS),
    })
  );
}

async function navigateWithCommands(page: Page) {
  await setupWithExtendedCommands(page);
  await page.goto("/");
  await page.evaluate(() => {
    sessionStorage.setItem("opman_token", "mock-jwt-token");
  });
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });
}

// ── Tests ─────────────────────────────────────────────

test.describe("Slash Command Popover", () => {
  test.describe("Triggering the popover", () => {
    test("popover appears when typing / in empty textarea", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });
    });

    test("popover shows multiple command items", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // Should have at least the built-in commands
      const items = popover.locator(".slash-popover-item");
      const count = await items.count();
      expect(count).toBeGreaterThanOrEqual(10);
    });

    test("popover does NOT appear when textarea already has text", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.fill("hello ");
      await textarea.press("/");

      // Popover should NOT appear since text isn't starting with /
      const popover = page.locator(".slash-popover");
      await expect(popover).not.toBeVisible({ timeout: 1_000 });
    });
  });

  test.describe("Command list rendering", () => {
    test("each command shows name with / prefix", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // First item should show /new (or similar built-in)
      const firstName = popover.locator(".slash-popover-name").first();
      const text = await firstName.textContent();
      expect(text).toMatch(/^\//);
    });

    test("commands with descriptions show the description", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // At least some descriptions should be visible
      const descs = popover.locator(".slash-popover-desc");
      const count = await descs.count();
      expect(count).toBeGreaterThan(0);
    });

    test("commands with args show the args hint", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // The /model command should show <model-name> or <model> args
      const argsHints = popover.locator(".slash-popover-args");
      const count = await argsHints.count();
      expect(count).toBeGreaterThan(0);
    });

    test("first command is selected by default", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      const firstItem = popover.locator(".slash-popover-item").first();
      await expect(firstItem).toHaveClass(/selected/);
    });
  });

  test.describe("Filtering", () => {
    test("typing after / filters commands by name", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      // Type /mod to filter to model/models
      await textarea.fill("/mod");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      const items = popover.locator(".slash-popover-item");
      const count = await items.count();
      // Should match "model" and "models" at minimum
      expect(count).toBeGreaterThanOrEqual(1);
      expect(count).toBeLessThanOrEqual(5);

      // All visible items should contain "mod" in their name
      for (let i = 0; i < count; i++) {
        const name = await items.nth(i).locator(".slash-popover-name").textContent();
        expect(name?.toLowerCase()).toContain("mod");
      }
    });

    test("filter narrows from full list to subset", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      const fullCount = await popover.locator(".slash-popover-item").count();

      // Now filter to "und"
      await textarea.fill("/und");
      const filteredCount = await popover.locator(".slash-popover-item").count();
      expect(filteredCount).toBeLessThan(fullCount);
      expect(filteredCount).toBeGreaterThanOrEqual(1);
    });

    test("popover hides when filter matches nothing", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.fill("/zzzznonexistent");

      // Popover should not be visible since filtered.length === 0 returns null
      const popover = page.locator(".slash-popover");
      await expect(popover).not.toBeVisible({ timeout: 2_000 });
    });
  });

  test.describe("Keyboard navigation", () => {
    test("ArrowDown moves selection to next item", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // First item selected
      const items = popover.locator(".slash-popover-item");
      await expect(items.nth(0)).toHaveClass(/selected/);

      // Press ArrowDown
      await page.keyboard.press("ArrowDown");
      await expect(items.nth(1)).toHaveClass(/selected/);
      await expect(items.nth(0)).not.toHaveClass(/selected/);
    });

    test("ArrowUp moves selection to previous item", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // Move down first
      await page.keyboard.press("ArrowDown");
      const items = popover.locator(".slash-popover-item");
      await expect(items.nth(1)).toHaveClass(/selected/);

      // Move back up
      await page.keyboard.press("ArrowUp");
      await expect(items.nth(0)).toHaveClass(/selected/);
    });

    test("Enter selects the highlighted command", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // Press Enter to select first command (should be "new" - a no-arg command)
      await page.keyboard.press("Enter");

      // Popover should close
      await expect(popover).not.toBeVisible({ timeout: 3_000 });
    });

    test("Tab selects the highlighted command", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // Press Tab to select
      await page.keyboard.press("Tab");

      // Popover should close
      await expect(popover).not.toBeVisible({ timeout: 3_000 });
    });
  });

  test.describe("Command selection behavior", () => {
    test("selecting a no-arg command clears textarea", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      // Type /undo (a no-arg command)
      await textarea.fill("/undo");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // Press Enter
      await page.keyboard.press("Enter");

      // Textarea should be cleared (no-arg commands execute immediately)
      await expect(textarea).toHaveValue("");
    });

    test("selecting a command with args leaves /<command> in textarea", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      // Type /mod to get /model (which takes args)
      await textarea.fill("/model");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // Find the "model" item (not "models") and click it
      const modelItem = popover.locator(".slash-popover-item", { hasText: "/model" }).first();
      await modelItem.click();

      // For arg commands, textarea should have "/model " (with trailing space)
      await expect(textarea).toHaveValue("/model ");
    });
  });

  test.describe("Closing the popover", () => {
    test("Escape closes the popover", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      await page.keyboard.press("Escape");
      await expect(popover).not.toBeVisible({ timeout: 2_000 });
    });

    test("popover closes when text no longer starts with /", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // Clear and type normal text
      await textarea.fill("hello");
      await expect(popover).not.toBeVisible({ timeout: 2_000 });
    });

    test("popover closes when space is typed after command", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.fill("/new");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // Type a space — includes space so it triggers setShowSlash(false)
      await textarea.fill("/new ");
      await expect(popover).not.toBeVisible({ timeout: 2_000 });
    });
  });

  test.describe("Mouse interaction", () => {
    test("clicking a command selects it", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      // Click on the "compact" command (no-arg → clears textarea)
      const compactItem = popover.locator(".slash-popover-item", { hasText: "/compact" });
      await compactItem.click();

      // Popover should close
      await expect(popover).not.toBeVisible({ timeout: 2_000 });
    });

    test("hovering a command highlights it", async ({ page }) => {
      await navigateWithCommands(page);

      const textarea = page.locator(".prompt-textarea");
      await textarea.focus();
      await textarea.press("/");

      const popover = page.locator(".slash-popover");
      await expect(popover).toBeVisible({ timeout: 3_000 });

      const items = popover.locator(".slash-popover-item");
      // First should be selected
      await expect(items.nth(0)).toHaveClass(/selected/);

      // Hover third item
      await items.nth(2).hover();
      await expect(items.nth(2)).toHaveClass(/selected/);
      await expect(items.nth(0)).not.toHaveClass(/selected/);
    });
  });
});
