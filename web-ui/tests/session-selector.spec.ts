/**
 * Playwright tests for the Session Selector Modal.
 *
 * Covers:
 *  - Opening via Cmd+Shift+S
 *  - Modal renders with header, search input, results, footer
 *  - Shows all sessions across projects
 *  - Current session marked with "current" badge
 *  - Search/filter by session title
 *  - Search/filter by project name
 *  - Keyboard navigation (ArrowDown, ArrowUp, Enter)
 *  - Selecting a session closes modal
 *  - Mouse interaction (click, hover)
 *  - Escape / backdrop close
 *  - Empty state
 *  - Session count display
 */

import { test, expect, Page } from "@playwright/test";
import { SESSION_ID, SESSION_ID_2, MOCK_APP_STATE, setupMockAPI } from "./helpers";

// ── Multi-project mock for richer tests ───────────────

const MULTI_PROJECT_STATE = {
  ...MOCK_APP_STATE,
  projects: [
    {
      name: "my-project",
      path: "/home/user/my-project",
      index: 0,
      active_session: SESSION_ID,
      sessions: [
        {
          id: SESSION_ID,
          title: "Test Session",
          parentID: "",
          directory: "/home/user/my-project",
          time: { created: 1700000000, updated: 1700001000 },
        },
        {
          id: SESSION_ID_2,
          title: "Another Session",
          parentID: "",
          directory: "/home/user/my-project",
          time: { created: 1700000500, updated: 1700001500 },
        },
      ],
      git_branch: "main",
      busy_sessions: [],
    },
    {
      name: "side-project",
      path: "/home/user/side-project",
      index: 1,
      active_session: "ses_side_001",
      sessions: [
        {
          id: "ses_side_001",
          title: "Feature Work",
          parentID: "",
          directory: "/home/user/side-project",
          time: { created: 1700002000, updated: 1700003000 },
        },
      ],
      git_branch: "dev",
      busy_sessions: [],
    },
  ],
};

async function navigateWithMultiProject(page: Page) {
  await setupMockAPI(page);
  // Override /api/state with multi-project state (last registered wins)
  await page.route("**/api/state", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MULTI_PROJECT_STATE),
    })
  );

  await page.goto("/");
  await page.evaluate(() => {
    sessionStorage.setItem("opman_token", "mock-jwt-token");
  });
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });
}

async function openSessionSelector(page: Page): Promise<ReturnType<Page["locator"]>> {
  await page.keyboard.press("Meta+Shift+s");
  const modal = page.locator('[role="dialog"][aria-label="Select session"]');
  await expect(modal).toBeVisible({ timeout: 5_000 });
  return modal;
}

// ── Tests ─────────────────────────────────────────────

test.describe("Session Selector Modal", () => {
  test.describe("Opening and closing", () => {
    test("opens via Cmd+Shift+S", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);
      await expect(modal).toBeVisible();
    });

    test("Escape closes the modal", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);
      await page.keyboard.press("Escape");
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });

    test("clicking backdrop closes the modal", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);
      await page.locator(".modal-backdrop").click({ position: { x: 5, y: 5 } });
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });

    test("close button closes the modal", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);
      await modal.locator('[aria-label="Close session selector"]').click();
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });
  });

  test.describe("Modal structure and rendering", () => {
    test("displays 'Select Session' header", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);
      await expect(modal.locator(".session-selector-header")).toContainText("Select Session");
    });

    test("shows session count", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);
      // 3 sessions total
      await expect(modal.locator(".session-selector-count")).toContainText("3 sessions");
    });

    test("search input is auto-focused", async ({ page }) => {
      await navigateWithMultiProject(page);
      await openSessionSelector(page);
      const input = page.locator(".session-selector-input");
      await expect(input).toBeFocused();
    });

    test("shows keyboard hints in footer", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);
      const footer = modal.locator(".session-selector-footer");
      await expect(footer).toContainText("Navigate");
      await expect(footer).toContainText("Select");
      await expect(footer).toContainText("Close");
    });
  });

  test.describe("Session list rendering", () => {
    test("lists all sessions across projects", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);
      const items = modal.locator(".session-selector-item");
      await expect(items).toHaveCount(3);
    });

    test("each session shows project name and title", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      // Check that at least one session has project name visible
      const projectNames = modal.locator(".session-selector-project");
      const count = await projectNames.count();
      expect(count).toBe(3);
    });

    test("current session has 'current' badge", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      // The active session (SESSION_ID) should have `.current` class and badge
      const currentItem = modal.locator(".session-selector-item.current");
      await expect(currentItem).toHaveCount(1);
      await expect(currentItem.locator(".session-selector-badge")).toContainText("current");
    });

    test("sessions show relative timestamps", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      const times = modal.locator(".session-selector-time");
      const count = await times.count();
      expect(count).toBe(3);
      // Timestamps should have some text (relative time)
      for (let i = 0; i < count; i++) {
        const text = await times.nth(i).textContent();
        expect(text?.length).toBeGreaterThan(0);
      }
    });

    test("first session is selected (highlighted) by default", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);
      const firstItem = modal.locator(".session-selector-item").first();
      await expect(firstItem).toHaveClass(/selected/);
    });
  });

  test.describe("Filtering", () => {
    test("typing filters sessions by title", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      await page.locator(".session-selector-input").fill("Feature");
      const items = modal.locator(".session-selector-item");
      await expect(items).toHaveCount(1);
      await expect(items.first().locator(".session-selector-title")).toContainText("Feature Work");
    });

    test("typing filters sessions by project name", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      await page.locator(".session-selector-input").fill("side-project");
      const items = modal.locator(".session-selector-item");
      await expect(items).toHaveCount(1);
    });

    test("session count updates with filter", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      await expect(modal.locator(".session-selector-count")).toContainText("3 sessions");
      // "Feature" matches only "Feature Work" session from side-project
      await page.locator(".session-selector-input").fill("Feature");
      await expect(modal.locator(".session-selector-count")).toContainText("1 session");
    });

    test("shows 'No matching sessions' when filter matches nothing", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      await page.locator(".session-selector-input").fill("zzzznonexistent");
      const empty = modal.locator(".session-selector-empty");
      await expect(empty).toContainText("No matching sessions");
    });
  });

  test.describe("Keyboard navigation", () => {
    test("ArrowDown moves selection", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      const items = modal.locator(".session-selector-item");
      await expect(items.nth(0)).toHaveClass(/selected/);

      await page.keyboard.press("ArrowDown");
      await expect(items.nth(1)).toHaveClass(/selected/);
      await expect(items.nth(0)).not.toHaveClass(/selected/);
    });

    test("ArrowUp moves selection up", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      await page.keyboard.press("ArrowDown");
      const items = modal.locator(".session-selector-item");
      await expect(items.nth(1)).toHaveClass(/selected/);

      await page.keyboard.press("ArrowUp");
      await expect(items.nth(0)).toHaveClass(/selected/);
    });

    test("Enter selects session and closes modal", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      await page.keyboard.press("Enter");
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });

    test("ArrowDown wraps around to first item", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      const items = modal.locator(".session-selector-item");
      // 3 items, press down 3 times to wrap
      await page.keyboard.press("ArrowDown");
      await page.keyboard.press("ArrowDown");
      await page.keyboard.press("ArrowDown");

      // Should wrap to first item (modulo logic)
      await expect(items.nth(0)).toHaveClass(/selected/);
    });
  });

  test.describe("Mouse interaction", () => {
    test("clicking a session selects it and closes modal", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      await modal.locator(".session-selector-item").nth(1).click();
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });

    test("hovering a session highlights it", async ({ page }) => {
      await navigateWithMultiProject(page);
      const modal = await openSessionSelector(page);

      const items = modal.locator(".session-selector-item");
      await expect(items.nth(0)).toHaveClass(/selected/);

      await items.nth(2).hover();
      await expect(items.nth(2)).toHaveClass(/selected/);
      await expect(items.nth(0)).not.toHaveClass(/selected/);
    });
  });
});
