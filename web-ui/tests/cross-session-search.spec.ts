/**
 * Playwright tests for the Cross-Session Search Modal.
 *
 * Covers:
 *  - Opening via Cmd+Shift+F
 *  - Modal renders with correct structure (header, input, body)
 *  - Search input is auto-focused
 *  - Hint text shown when no query entered
 *  - Typing a query triggers debounced API search
 *  - Results grouped by session
 *  - Result items show role, snippet, timestamp
 *  - Highlighted search matches in snippets
 *  - Empty state when no results
 *  - Error state when API fails
 *  - Loading spinner during search
 *  - Keyboard navigation (ArrowDown, ArrowUp)
 *  - Enter navigates to selected result
 *  - Click navigates to a result
 *  - Escape closes the modal
 *  - Clicking backdrop closes the modal
 *  - "Showing X of Y results" truncation notice
 */

import { test, expect, Page } from "@playwright/test";
import { SESSION_ID, SESSION_ID_2, setupMockAPI } from "./helpers";

// ── Mock search results ───────────────────────────────

const MOCK_SEARCH_RESULTS = {
  query: "hello",
  results: [
    {
      session_id: SESSION_ID,
      session_title: "Test Session",
      project_name: "my-project",
      message_id: "msg_001",
      role: "user",
      snippet: "Hello, how are you?",
      timestamp: 1700000100,
    },
    {
      session_id: SESSION_ID,
      session_title: "Test Session",
      project_name: "my-project",
      message_id: "msg_005",
      role: "assistant",
      snippet: "Hello! I can help you with that.",
      timestamp: 1700000150,
    },
    {
      session_id: SESSION_ID_2,
      session_title: "Another Session",
      project_name: "my-project",
      message_id: "msg_010",
      role: "user",
      snippet: "Say hello world in python",
      timestamp: 1700000500,
    },
  ],
  total: 3,
};

const MOCK_SEARCH_EMPTY = {
  query: "nonexistent",
  results: [],
  total: 0,
};

const MOCK_SEARCH_TRUNCATED = {
  query: "the",
  results: Array.from({ length: 50 }, (_, i) => ({
    session_id: SESSION_ID,
    session_title: "Test Session",
    project_name: "my-project",
    message_id: `msg_${i}`,
    role: i % 2 === 0 ? "user" : "assistant",
    snippet: `Result #${i}: the quick brown fox`,
    timestamp: 1700000100 + i,
  })),
  total: 120, // More than 50 results
};

async function setupSearchMock(page: Page, response: object) {
  // Register this AFTER setupMockAPI so it takes priority (last registered wins)
  await page.route("**/api/project/*/search*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(response),
    })
  );
}

async function navigateAndOpenSearch(page: Page, searchResponse?: object) {
  await setupMockAPI(page);
  if (searchResponse) {
    await setupSearchMock(page, searchResponse);
  } else {
    await setupSearchMock(page, MOCK_SEARCH_RESULTS);
  }

  await page.goto("/");
  await page.evaluate(() => {
    sessionStorage.setItem("opman_token", "mock-jwt-token");
  });
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });

  // Open cross-session search with Cmd+Shift+F
  await page.keyboard.press("Meta+Shift+f");
  const modal = page.locator(".cross-session-search-modal");
  await expect(modal).toBeVisible({ timeout: 5_000 });
  return modal;
}

// ── Tests ─────────────────────────────────────────────

test.describe("Cross-Session Search Modal", () => {
  test.describe("Opening and closing", () => {
    test("opens via Cmd+Shift+F keyboard shortcut", async ({ page }) => {
      await setupMockAPI(page);
      await setupSearchMock(page, MOCK_SEARCH_RESULTS);

      await page.goto("/");
      await page.evaluate(() => {
        sessionStorage.setItem("opman_token", "mock-jwt-token");
      });
      await page.reload();
      await page.waitForSelector(".chat-layout", { timeout: 10_000 });

      await page.keyboard.press("Meta+Shift+f");
      const modal = page.locator('.cross-session-search-modal[role="dialog"]');
      await expect(modal).toBeVisible({ timeout: 5_000 });
    });

    test("Escape closes the modal", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);
      await page.keyboard.press("Escape");
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });

    test("clicking backdrop closes the modal", async ({ page }) => {
      await navigateAndOpenSearch(page);
      const modal = page.locator(".cross-session-search-modal");
      await expect(modal).toBeVisible();

      // Click the backdrop
      await page.locator(".modal-backdrop").click({ position: { x: 5, y: 5 } });
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });
  });

  test.describe("Modal structure", () => {
    test("search input is auto-focused", async ({ page }) => {
      await navigateAndOpenSearch(page);
      const input = page.locator(".cross-session-search-input");
      await expect(input).toBeFocused();
    });

    test("shows hint text when query is empty", async ({ page }) => {
      await navigateAndOpenSearch(page);
      const hint = page.locator(".cross-session-search-hint");
      await expect(hint).toBeVisible();
      await expect(hint).toContainText("Type to search");
    });

    test("search input has correct placeholder", async ({ page }) => {
      await navigateAndOpenSearch(page);
      const input = page.locator(".cross-session-search-input");
      await expect(input).toHaveAttribute("placeholder", "Search across all sessions...");
    });
  });

  test.describe("Searching and results", () => {
    test("typing a query shows search results after debounce", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      const input = page.locator(".cross-session-search-input");
      await input.fill("hello");

      // Results should appear after debounce (300ms)
      const results = modal.locator(".cross-session-search-result");
      await expect(results.first()).toBeVisible({ timeout: 3_000 });
      await expect(results).toHaveCount(3);
    });

    test("results are grouped by session", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      await page.locator(".cross-session-search-input").fill("hello");

      // Wait for results
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      // Should have 2 groups (Test Session and Another Session)
      const groups = modal.locator(".cross-session-search-group");
      await expect(groups).toHaveCount(2);
    });

    test("session group headers show session title and match count", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      await page.locator(".cross-session-search-input").fill("hello");
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      // First group header should show "Test Session" with "2 matches"
      const firstGroupHeader = modal.locator(".cross-session-search-group-header").first();
      await expect(firstGroupHeader.locator(".cross-session-search-session-title")).toContainText("Test Session");
      await expect(firstGroupHeader.locator(".cross-session-search-match-count")).toContainText("2 matches");
    });

    test("result items show role (user/assistant)", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      await page.locator(".cross-session-search-input").fill("hello");
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      const firstRole = modal.locator(".cross-session-search-role").first();
      await expect(firstRole).toContainText("user");
    });

    test("result items show snippet text", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      await page.locator(".cross-session-search-input").fill("hello");
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      const firstSnippet = modal.locator(".cross-session-search-snippet").first();
      await expect(firstSnippet).toContainText("Hello, how are you?");
    });

    test("snippet highlights query matches", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      await page.locator(".cross-session-search-input").fill("hello");
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      // Should have <mark class="search-highlight"> elements
      const highlights = modal.locator(".search-highlight");
      const count = await highlights.count();
      expect(count).toBeGreaterThanOrEqual(1);
    });

    test("first result is selected by default", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      await page.locator(".cross-session-search-input").fill("hello");
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      const firstResult = modal.locator(".cross-session-search-result").first();
      await expect(firstResult).toHaveClass(/selected/);
    });
  });

  test.describe("Empty and error states", () => {
    test("shows 'No results found' when search returns empty", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page, MOCK_SEARCH_EMPTY);

      await page.locator(".cross-session-search-input").fill("nonexistent");

      const empty = modal.locator(".cross-session-search-empty");
      await expect(empty).toBeVisible({ timeout: 3_000 });
      await expect(empty).toContainText('No results found for "nonexistent"');
    });

    test("shows error message when search API fails", async ({ page }) => {
      await setupMockAPI(page);
      await page.route("**/api/project/*/search*", (route) =>
        route.fulfill({ status: 500, contentType: "application/json", body: JSON.stringify({ error: "Internal" }) })
      );

      await page.goto("/");
      await page.evaluate(() => {
        sessionStorage.setItem("opman_token", "mock-jwt-token");
      });
      await page.reload();
      await page.waitForSelector(".chat-layout", { timeout: 10_000 });

      await page.keyboard.press("Meta+Shift+f");
      const modal = page.locator(".cross-session-search-modal");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      await page.locator(".cross-session-search-input").fill("test");

      const error = modal.locator(".cross-session-search-error");
      await expect(error).toBeVisible({ timeout: 3_000 });
    });
  });

  test.describe("Truncation notice", () => {
    test("shows 'Showing X of Y results' when results exceed limit", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page, MOCK_SEARCH_TRUNCATED);

      await page.locator(".cross-session-search-input").fill("the");

      // Wait for results
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      const more = modal.locator(".cross-session-search-more");
      await expect(more).toBeVisible();
      await expect(more).toContainText("Showing 50 of 120 results");
    });
  });

  test.describe("Keyboard navigation", () => {
    test("ArrowDown moves selection to next result", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      await page.locator(".cross-session-search-input").fill("hello");
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      const results = modal.locator(".cross-session-search-result");
      await expect(results.nth(0)).toHaveClass(/selected/);

      await page.keyboard.press("ArrowDown");
      await expect(results.nth(1)).toHaveClass(/selected/);
      await expect(results.nth(0)).not.toHaveClass(/selected/);
    });

    test("ArrowUp moves selection to previous result", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      await page.locator(".cross-session-search-input").fill("hello");
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      const results = modal.locator(".cross-session-search-result");
      await page.keyboard.press("ArrowDown");
      await expect(results.nth(1)).toHaveClass(/selected/);

      await page.keyboard.press("ArrowUp");
      await expect(results.nth(0)).toHaveClass(/selected/);
    });

    test("Enter on selected result closes modal (navigates)", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      await page.locator(".cross-session-search-input").fill("hello");
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      await page.keyboard.press("Enter");
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });
  });

  test.describe("Mouse interaction", () => {
    test("clicking a result navigates and closes modal", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      await page.locator(".cross-session-search-input").fill("hello");
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      await modal.locator(".cross-session-search-result").first().click();
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });

    test("hovering a result highlights it", async ({ page }) => {
      const modal = await navigateAndOpenSearch(page);

      await page.locator(".cross-session-search-input").fill("hello");
      await expect(modal.locator(".cross-session-search-result").first()).toBeVisible({ timeout: 3_000 });

      const results = modal.locator(".cross-session-search-result");
      await expect(results.nth(0)).toHaveClass(/selected/);

      await results.nth(2).hover();
      await expect(results.nth(2)).toHaveClass(/selected/);
      await expect(results.nth(0)).not.toHaveClass(/selected/);
    });
  });
});
