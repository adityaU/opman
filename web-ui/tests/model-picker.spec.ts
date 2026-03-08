/**
 * Playwright tests for the Model Picker Modal.
 *
 * Covers:
 *  - Opening via Cmd+' keyboard shortcut
 *  - Opening via the model chip in the prompt input
 *  - Modal renders with correct structure (header, search, tabs, results)
 *  - Model list shows providers and model names
 *  - Search/filter functionality
 *  - Tab switching between Connected / All Providers
 *  - Keyboard navigation (ArrowDown, ArrowUp, Enter)
 *  - Selecting a model
 *  - Closing via Escape
 *  - Closing via backdrop click
 *  - Loading / error / empty states
 */

import { test, expect, Page } from "@playwright/test";
import {
  SESSION_ID,
  MOCK_APP_STATE,
  MOCK_PROVIDERS,
  MOCK_THEME,
  setupMockAPI,
  navigateAuthenticated,
} from "./helpers";

// ── Rich provider mock for model-picker tests ─────────

const RICH_PROVIDERS = {
  all: [
    {
      id: "anthropic",
      name: "Anthropic",
      models: {
        "claude-sonnet-4-20250514": {
          name: "Claude Sonnet 4",
          id: "claude-sonnet-4-20250514",
          limit: { context: 200000, output: 64000 },
          features: ["reasoning"],
        },
        "claude-opus-4-20250514": {
          name: "Claude Opus 4",
          id: "claude-opus-4-20250514",
          limit: { context: 200000, output: 32000 },
          features: ["reasoning"],
        },
      },
    },
    {
      id: "openai",
      name: "OpenAI",
      models: {
        "gpt-4o": {
          name: "GPT-4o",
          id: "gpt-4o",
          limit: { context: 128000, output: 16000 },
          features: [],
        },
        "o3-mini": {
          name: "o3-mini",
          id: "o3-mini",
          limit: { context: 128000, output: 65000 },
          features: ["reasoning"],
        },
      },
    },
  ],
  connected: ["anthropic"],
  default: { anthropic: "claude-sonnet-4-20250514" },
};

// Helper: override the /api/providers route with the rich shape
async function setupRichProviders(page: Page) {
  await page.route("**/api/providers", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(RICH_PROVIDERS),
    })
  );
}

// Helper: navigate with rich providers mock
async function navigateWithPicker(page: Page) {
  // Register shared mocks first, then override /api/providers with rich shape.
  // Playwright matches routes in REVERSE registration order (last = highest
  // priority), so the rich-providers route must come AFTER setupMockAPI.
  await setupMockAPI(page);
  await setupRichProviders(page);

  await page.goto("/");
  await page.evaluate(() => {
    sessionStorage.setItem("opman_token", "mock-jwt-token");
  });
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });
}

// ── Tests ─────────────────────────────────────────────

test.describe("Model Picker Modal", () => {
  test.beforeEach(async ({ page }) => {
    // Invalidate singleton provider cache between tests
    await page.addInitScript(() => {
      (window as any).__INVALIDATE_PROVIDER_CACHE = true;
    });
  });

  test.describe("Opening the modal", () => {
    test("opens via Cmd+' keyboard shortcut", async ({ page }) => {
      await navigateWithPicker(page);

      // Press Cmd+'
      await page.keyboard.press("Meta+'");
      const modal = page.locator('[role="dialog"][aria-label="Choose model"]');
      await expect(modal).toBeVisible({ timeout: 5_000 });
    });

    test("opens via model chip in the prompt input", async ({ page }) => {
      await navigateWithPicker(page);

      // Click the model chip button (has Cpu icon)
      const chip = page.locator(".prompt-chip").first();
      await chip.click();

      const modal = page.locator('[role="dialog"][aria-label="Choose model"]');
      await expect(modal).toBeVisible({ timeout: 5_000 });
    });
  });

  test.describe("Modal structure and rendering", () => {
    test("displays header with 'Choose Model' text", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      const header = modal.locator(".model-picker-header");
      await expect(header).toContainText("Choose Model");
    });

    test("displays model count in header", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Connected tab is active by default, shows only Anthropic (2 models)
      const count = modal.locator(".model-picker-count");
      await expect(count).toContainText("2 models");
    });

    test("search input is auto-focused", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      const input = modal.locator(".model-picker-input");
      await expect(input).toBeFocused();
    });

    test("shows Connected and All Providers tabs", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      const tabs = modal.locator(".model-picker-tab");
      await expect(tabs).toHaveCount(2);
      await expect(tabs.nth(0)).toContainText("Connected");
      await expect(tabs.nth(1)).toContainText("All Providers");
    });

    test("Connected tab is active by default", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      const connectedTab = modal.locator(".model-picker-tab").nth(0);
      await expect(connectedTab).toHaveClass(/active/);
    });
  });

  test.describe("Model list rendering", () => {
    test("shows connected provider models by default", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      const items = modal.locator(".model-picker-item");
      // Only Anthropic is connected → 2 models
      await expect(items).toHaveCount(2);
    });

    test("displays model name and provider name", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Check first item has model and provider info
      const firstItem = modal.locator(".model-picker-item").first();
      await expect(firstItem.locator(".model-picker-name")).toBeVisible();
      await expect(firstItem.locator(".model-picker-provider")).toContainText("Anthropic");
    });

    test("default model shows check icon", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Default model is claude-sonnet-4, should be sorted first and have check icon
      const defaultItem = modal.locator(".model-picker-item").first();
      await expect(defaultItem.locator(".model-default-icon")).toBeVisible();
    });

    test("shows context window and output limit badges", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Anthropic models have 200K context
      const firstItem = modal.locator(".model-picker-item").first();
      await expect(firstItem.locator(".model-picker-ctx")).toContainText("200K ctx");
      await expect(firstItem.locator(".model-picker-out")).toBeVisible();
    });

    test("first item is selected (highlighted) by default", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      const firstItem = modal.locator(".model-picker-item").first();
      await expect(firstItem).toHaveClass(/selected/);
    });
  });

  test.describe("Tab switching", () => {
    test("Tab key switches to All Providers tab", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Initially on Connected tab with 2 models
      await expect(modal.locator(".model-picker-item")).toHaveCount(2);

      // Press Tab to switch to All Providers
      await page.keyboard.press("Tab");

      const allTab = modal.locator(".model-picker-tab").nth(1);
      await expect(allTab).toHaveClass(/active/);

      // Now should show all 4 models (2 Anthropic + 2 OpenAI)
      await expect(modal.locator(".model-picker-item")).toHaveCount(4);
    });

    test("clicking All Providers tab shows all models", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Click All Providers tab
      await modal.locator(".model-picker-tab").nth(1).click();

      // Should show 4 models
      await expect(modal.locator(".model-picker-item")).toHaveCount(4);
    });

    test("clicking Connected tab filters to connected providers only", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Switch to All first
      await modal.locator(".model-picker-tab").nth(1).click();
      await expect(modal.locator(".model-picker-item")).toHaveCount(4);

      // Switch back to Connected
      await modal.locator(".model-picker-tab").nth(0).click();
      await expect(modal.locator(".model-picker-item")).toHaveCount(2);
    });
  });

  test.describe("Search / filter", () => {
    test("typing filters models by name", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Switch to All Providers first so we have 4 models
      await page.keyboard.press("Tab");
      await expect(modal.locator(".model-picker-item")).toHaveCount(4);

      // Type "opus" to filter
      await page.keyboard.type("opus");
      await expect(modal.locator(".model-picker-item")).toHaveCount(1);
      await expect(modal.locator(".model-picker-name").first()).toContainText("Claude Opus 4");
    });

    test("typing filters models by provider name", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Switch to All Providers
      await page.keyboard.press("Tab");
      await expect(modal.locator(".model-picker-item")).toHaveCount(4);

      // Type "openai" to filter by provider
      await page.keyboard.type("openai");
      await expect(modal.locator(".model-picker-item")).toHaveCount(2);
    });

    test("shows 'No models found' when filter matches nothing on All tab", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Switch to All Providers
      await page.keyboard.press("Tab");

      await page.keyboard.type("nonexistentmodel");
      const empty = modal.locator(".model-picker-empty");
      await expect(empty).toContainText("No models found");
    });

    test("model count updates as filter narrows results", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Switch to All Providers to have 4 models
      await page.keyboard.press("Tab");
      await expect(modal.locator(".model-picker-count")).toContainText("4 models");

      await page.keyboard.type("gpt");
      await expect(modal.locator(".model-picker-count")).toContainText("1 model");
    });
  });

  test.describe("Keyboard navigation", () => {
    test("ArrowDown moves selection down", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // First item should be selected
      const items = modal.locator(".model-picker-item");
      await expect(items.nth(0)).toHaveClass(/selected/);

      // Press ArrowDown
      await page.keyboard.press("ArrowDown");
      await expect(items.nth(1)).toHaveClass(/selected/);
      await expect(items.nth(0)).not.toHaveClass(/selected/);
    });

    test("ArrowUp moves selection up", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Move down first then back up
      await page.keyboard.press("ArrowDown");
      const items = modal.locator(".model-picker-item");
      await expect(items.nth(1)).toHaveClass(/selected/);

      await page.keyboard.press("ArrowUp");
      await expect(items.nth(0)).toHaveClass(/selected/);
    });

    test("Enter selects the highlighted model and closes modal", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Press Enter to select first model
      await page.keyboard.press("Enter");

      // Modal should close
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });

    test("ArrowDown does not go past last item", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // 2 connected models; press ArrowDown 10 times
      for (let i = 0; i < 10; i++) {
        await page.keyboard.press("ArrowDown");
      }

      const items = modal.locator(".model-picker-item");
      await expect(items.nth(1)).toHaveClass(/selected/);
    });
  });

  test.describe("Closing the modal", () => {
    test("Escape closes the modal", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator('[role="dialog"][aria-label="Choose model"]');
      await expect(modal).toBeVisible({ timeout: 5_000 });

      await page.keyboard.press("Escape");
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });

    test("clicking backdrop closes the modal", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator('[role="dialog"][aria-label="Choose model"]');
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Click the backdrop (outside the modal)
      await page.locator(".modal-backdrop").click({ position: { x: 10, y: 10 } });
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });
  });

  test.describe("Loading and error states", () => {
    test("shows loading state when providers are fetching", async ({ page }) => {
      // Register shared mocks first
      await setupMockAPI(page);
      // Override with a slow providers response (last registered = highest priority)
      await page.route("**/api/providers", async (route) => {
        await new Promise((r) => setTimeout(r, 3000));
        await route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify(RICH_PROVIDERS),
        });
      });

      await page.goto("/");
      await page.evaluate(() => {
        sessionStorage.setItem("opman_token", "mock-jwt-token");
      });
      await page.reload();
      await page.waitForSelector(".chat-layout", { timeout: 10_000 });

      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Should show loading text
      const loading = modal.locator(".model-picker-empty");
      await expect(loading).toContainText("Loading providers");
    });

    test("shows error state when provider fetch fails", async ({ page }) => {
      // Register shared mocks first
      await setupMockAPI(page);
      // Override with error response (last registered = highest priority)
      await page.route("**/api/providers", (route) =>
        route.fulfill({ status: 500, contentType: "application/json", body: JSON.stringify({ error: "Server error" }) })
      );

      await page.goto("/");
      await page.evaluate(() => {
        sessionStorage.setItem("opman_token", "mock-jwt-token");
      });
      await page.reload();
      await page.waitForSelector(".chat-layout", { timeout: 10_000 });

      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      // Should show error
      const error = modal.locator(".model-picker-error");
      await expect(error).toBeVisible();
    });
  });

  test.describe("Refresh button", () => {
    test("refresh button is visible and has correct aria-label", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      const refreshBtn = modal.locator('[aria-label="Refresh providers"]');
      await expect(refreshBtn).toBeVisible();
    });
  });

  test.describe("Mouse interaction", () => {
    test("hovering a model item highlights it", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      const items = modal.locator(".model-picker-item");
      // First item should be selected initially
      await expect(items.nth(0)).toHaveClass(/selected/);

      // Hover second item
      await items.nth(1).hover();
      await expect(items.nth(1)).toHaveClass(/selected/);
      await expect(items.nth(0)).not.toHaveClass(/selected/);
    });

    test("clicking a model item selects it and closes the modal", async ({ page }) => {
      await navigateWithPicker(page);
      await page.keyboard.press("Meta+'");
      const modal = page.locator(".model-picker");
      await expect(modal).toBeVisible({ timeout: 5_000 });

      const secondItem = modal.locator(".model-picker-item").nth(1);
      await secondItem.click();

      // Modal should close
      await expect(modal).not.toBeVisible({ timeout: 3_000 });
    });
  });
});
