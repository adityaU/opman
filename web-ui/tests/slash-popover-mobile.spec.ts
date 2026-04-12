import { test, expect } from "@playwright/test";
import { navigateAuthenticated } from "./helpers";

/**
 * Verify that the slash-command popover is visible and not clipped on mobile viewports.
 *
 * Root cause of the bug: .mobile-input-wrapper had overflow:hidden which clipped
 * the absolutely-positioned .slash-popover that renders above the input.
 */

test.describe("Slash command popover on mobile", () => {
  test.use({ viewport: { width: 375, height: 812 } }); // iPhone X dimensions

  test("slash popover is visible when typing /", async ({ page }) => {
    await navigateAuthenticated(page);

    // The prompt textarea should be visible
    const textarea = page.locator(".prompt-textarea");
    await expect(textarea).toBeVisible({ timeout: 10_000 });

    // Type "/" to trigger the slash popover
    await textarea.focus();
    await textarea.fill("/");

    // The slash popover should appear and be visible
    const popover = page.locator(".slash-popover");
    await expect(popover).toBeVisible({ timeout: 5_000 });

    // Verify the popover has items
    const items = popover.locator(".slash-popover-item");
    const count = await items.count();
    expect(count).toBeGreaterThan(0);

    // The popover should be positioned above the input, not clipped.
    // Check that the popover's bounding box is within the viewport.
    const box = await popover.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.y).toBeGreaterThanOrEqual(0); // top edge inside viewport
    expect(box!.y + box!.height).toBeLessThanOrEqual(812); // bottom edge inside viewport
    expect(box!.width).toBeGreaterThan(100); // has reasonable width

    // Verify first item is clickable (not behind an overflow clip)
    const firstItem = items.first();
    await expect(firstItem).toBeVisible();

    // Screenshot for visual verification
    await page.screenshot({
      path: "tests/screenshots/slash-popover-mobile.png",
      fullPage: false,
    });
  });

  test("slash popover items are selectable on mobile", async ({ page }) => {
    await navigateAuthenticated(page);

    const textarea = page.locator(".prompt-textarea");
    await expect(textarea).toBeVisible({ timeout: 10_000 });

    // Type "/" to trigger the popover
    await textarea.focus();
    await textarea.fill("/");

    const popover = page.locator(".slash-popover");
    await expect(popover).toBeVisible({ timeout: 5_000 });

    // Click the first command
    const firstItem = popover.locator(".slash-popover-item").first();
    const commandName = await firstItem.locator(".slash-popover-name").textContent();
    await firstItem.click();

    // After clicking, the popover should close
    await expect(popover).not.toBeVisible({ timeout: 3_000 });
  });

  test("slash popover filters as user types", async ({ page }) => {
    await navigateAuthenticated(page);

    const textarea = page.locator(".prompt-textarea");
    await expect(textarea).toBeVisible({ timeout: 10_000 });

    // Type "/mod" to filter commands
    await textarea.focus();
    await textarea.fill("/mod");

    const popover = page.locator(".slash-popover");
    await expect(popover).toBeVisible({ timeout: 5_000 });

    // Should show only commands matching "mod" (e.g., "model", "models")
    const items = popover.locator(".slash-popover-item");
    const count = await items.count();
    expect(count).toBeGreaterThan(0);

    // All visible items should contain "mod" in their name
    for (let i = 0; i < count; i++) {
      const name = await items.nth(i).locator(".slash-popover-name").textContent();
      expect(name?.toLowerCase()).toContain("mod");
    }
  });
});

test.describe("Slash command popover on desktop", () => {
  // Default viewport (1280x720)

  test("slash popover is visible on desktop too (regression)", async ({ page }) => {
    await navigateAuthenticated(page);

    const textarea = page.locator(".prompt-textarea");
    await expect(textarea).toBeVisible({ timeout: 10_000 });

    await textarea.focus();
    await textarea.fill("/");

    const popover = page.locator(".slash-popover");
    await expect(popover).toBeVisible({ timeout: 5_000 });

    const items = popover.locator(".slash-popover-item");
    const count = await items.count();
    expect(count).toBeGreaterThan(0);
  });
});
