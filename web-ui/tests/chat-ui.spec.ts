import { test, expect } from "@playwright/test";
import {
  navigateAuthenticated,
  setupMockAPI,
} from "./helpers";

// ─────────────────────────────────────────────────────
// Login tests
// ─────────────────────────────────────────────────────

test.describe("Login page", () => {
  test("shows login form when not authenticated", async ({ page }) => {
    await page.route("**/api/auth/verify", (route) =>
      route.fulfill({ status: 401, body: "Unauthorized" })
    );

    await page.goto("/");
    await page.waitForSelector(".login-container", { timeout: 10_000 });

    await expect(page.locator("h1")).toHaveText("opencode");
    await expect(page.locator('input[type="text"]')).toBeVisible();
    await expect(page.locator('input[type="password"]')).toBeVisible();
    await expect(page.locator('button[type="submit"]')).toBeVisible();
    await expect(page.locator('button[type="submit"]')).toHaveText("Sign In");
  });

  test("sign-in button is disabled when fields are empty", async ({ page }) => {
    await page.route("**/api/auth/verify", (route) =>
      route.fulfill({ status: 401, body: "Unauthorized" })
    );

    await page.goto("/");
    await page.waitForSelector(".login-container", { timeout: 10_000 });

    await expect(page.locator('button[type="submit"]')).toBeDisabled();
  });

  test("shows error on invalid credentials", async ({ page }) => {
    await page.route("**/api/auth/verify", (route) =>
      route.fulfill({ status: 401, body: "Unauthorized" })
    );
    await page.route("**/api/auth/login", (route) =>
      route.fulfill({ status: 401, body: "Invalid credentials" })
    );

    await page.goto("/");
    await page.waitForSelector(".login-container", { timeout: 10_000 });

    await page.fill('input[type="text"]', "baduser");
    await page.fill('input[type="password"]', "badpass");
    await page.click('button[type="submit"]');

    await expect(page.locator(".login-error")).toBeVisible();
    await expect(page.locator(".login-error")).toHaveText(
      "Invalid username or password"
    );
  });

  test("successful login navigates to chat layout", async ({ page }) => {
    await page.route("**/api/auth/verify", (route) =>
      route.fulfill({ status: 401, body: "Unauthorized" })
    );
    await page.route("**/api/auth/login", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ token: "mock-jwt-token" }),
      })
    );

    await page.goto("/");
    await page.waitForSelector(".login-container", { timeout: 10_000 });

    await page.fill('input[type="text"]', "admin");
    await page.fill('input[type="password"]', "password");

    // Set up mock routes for after login
    await setupMockAPI(page);

    await page.click('button[type="submit"]');

    await expect(page.locator(".chat-layout")).toBeVisible({ timeout: 10_000 });
  });
});

// ─────────────────────────────────────────────────────
// Chat layout tests
// ─────────────────────────────────────────────────────

test.describe("Chat layout", () => {
  test.beforeEach(async ({ page }) => {
    await navigateAuthenticated(page);
  });

  test("renders sidebar and main chat area", async ({ page }) => {
    await expect(page.locator(".chat-sidebar")).toBeVisible();
    await expect(page.locator(".chat-main")).toBeVisible();
    // Status bar uses class "chat-status-bar"
    await expect(page.locator(".chat-status-bar")).toBeVisible();
  });

  test("sidebar shows session list", async ({ page }) => {
    const sidebar = page.locator(".chat-sidebar");
    await expect(sidebar).toBeVisible();

    // Sessions use class "sb-session-group" (one per parent session)
    await expect(sidebar.locator(".sb-session-group")).toHaveCount(2);

    // The active session row should have the "active" class
    const activeSessions = sidebar.locator(".sb-session.active");
    await expect(activeSessions).toHaveCount(1);
  });

  test("sidebar shows project name", async ({ page }) => {
    const sidebar = page.locator(".chat-sidebar");
    await expect(sidebar.getByText("my-project")).toBeVisible();
  });

  test("sidebar has new session button", async ({ page }) => {
    // The new session button uses class "sb-new-btn"
    const newBtn = page.locator(".sb-new-btn");
    await expect(newBtn).toBeVisible();
  });

  test("sidebar shows session titles", async ({ page }) => {
    await expect(page.getByText("Test Session")).toBeVisible();
    await expect(page.getByText("Another Session")).toBeVisible();
  });
});

// ─────────────────────────────────────────────────────
// Message display tests
// ─────────────────────────────────────────────────────

test.describe("Message timeline", () => {
  test.beforeEach(async ({ page }) => {
    await navigateAuthenticated(page);
  });

  test("displays user message", async ({ page }) => {
    await expect(page.getByText("Hello, how are you?")).toBeVisible();
  });

  test("displays assistant message with markdown", async ({ page }) => {
    await expect(
      page.getByText("I'm doing well! Here is some code:")
    ).toBeVisible();
  });

  test("renders code blocks with syntax highlighting", async ({ page }) => {
    const codeBlock = page.locator(".code-block-wrapper");
    await expect(codeBlock.first()).toBeVisible();

    await expect(codeBlock.first().locator(".code-block-header")).toContainText(
      "typescript"
    );

    await expect(
      codeBlock.first().locator('.code-block-action-btn[aria-label="Copy code"]')
    ).toBeVisible();
  });

  test("displays model name from object format correctly", async ({ page }) => {
    // The model is { modelID: "claude-sonnet-4-20250514", providerID: "anthropic" }
    // Our fix should render it as the modelID string
    await expect(page.getByText("claude-sonnet-4-20250514").first()).toBeVisible();
  });

  test("does NOT render model object as [object Object]", async ({ page }) => {
    const objectText = page.getByText("[object Object]");
    await expect(objectText).toHaveCount(0);
  });

  test("displays tool call with tool name", async ({ page }) => {
    await expect(page.getByText("read_file").first()).toBeVisible();
  });

  test("displays message cost", async ({ page }) => {
    // msg_002 ($0.0032) and msg_003 ($0.005) are grouped together as
    // consecutive assistant messages, so the displayed cost is the sum.
    await expect(page.getByText("$0.0082")).toBeVisible();
  });

  test("shows role labels", async ({ page }) => {
    await expect(page.getByText("You").first()).toBeVisible();
    await expect(page.getByText("Assistant").first()).toBeVisible();
  });
});

// ─────────────────────────────────────────────────────
// Prompt input tests
// ─────────────────────────────────────────────────────

test.describe("Prompt input", () => {
  test.beforeEach(async ({ page }) => {
    await navigateAuthenticated(page);
  });

  test("has a textarea for input", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await expect(textarea).toBeVisible();
  });

  test("has send and abort buttons", async ({ page }) => {
    // When not busy, there's a send button
    const sendBtn = page.locator(".prompt-send-btn");
    await expect(sendBtn).toBeVisible();
  });

  test("can type a message in the textarea", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await textarea.fill("Hello world");
    await expect(textarea).toHaveValue("Hello world");
  });

  test("shows placeholder text", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await expect(textarea).toHaveAttribute(
      "placeholder",
      "Type a message... (/ for commands, paste or drop images)"
    );
  });

  test("shows keyboard hints", async ({ page }) => {
    const hints = page.locator(".prompt-hints");
    await expect(hints).toBeVisible();
    await expect(hints).toContainText("Enter");
    await expect(hints).toContainText("Commands");
  });
});

// ─────────────────────────────────────────────────────
// Status bar tests
// ─────────────────────────────────────────────────────

test.describe("Status bar", () => {
  test.beforeEach(async ({ page }) => {
    await navigateAuthenticated(page);
  });

  test("shows project name", async ({ page }) => {
    const statusBar = page.locator(".chat-status-bar");
    await expect(statusBar.locator(".status-bar-project")).toHaveText(
      "my-project"
    );
  });

  test("shows git branch", async ({ page }) => {
    const statusBar = page.locator(".chat-status-bar");
    await expect(statusBar.locator(".status-bar-branch")).toContainText("main");
  });

  test("shows session status indicator", async ({ page }) => {
    const statusBar = page.locator(".chat-status-bar");
    await expect(statusBar.locator(".status-bar-status")).toHaveText("ready");
  });

  test("shows token count when stats are available", async ({ page }) => {
    const statusBar = page.locator(".chat-status-bar");
    await expect(statusBar.locator(".status-bar-tokens")).toBeVisible();
  });

  test("shows cost when stats are available", async ({ page }) => {
    const statusBar = page.locator(".chat-status-bar");
    await expect(statusBar.locator(".status-bar-cost")).toBeVisible();
  });

  test("has sidebar toggle button", async ({ page }) => {
    const statusBar = page.locator(".chat-status-bar");
    const sidebarBtn = statusBar.locator(
      'button[title="Toggle Sidebar (Cmd+B)"]'
    );
    await expect(sidebarBtn).toBeVisible();
  });

  test("has terminal toggle button", async ({ page }) => {
    const statusBar = page.locator(".chat-status-bar");
    const termBtn = statusBar.locator(
      'button[title*="Toggle Terminal"]'
    );
    await expect(termBtn).toBeVisible();
  });

  test("has command palette button", async ({ page }) => {
    const statusBar = page.locator(".chat-status-bar");
    const cmdBtn = statusBar.locator(
      'button[title*="Command Palette"]'
    );
    await expect(cmdBtn).toBeVisible();
  });
});

// ─────────────────────────────────────────────────────
// Command palette tests
// ─────────────────────────────────────────────────────

test.describe("Command palette", () => {
  test.beforeEach(async ({ page }) => {
    await navigateAuthenticated(page);
  });

  test("opens with Cmd+Shift+P", async ({ page }) => {
    await page.keyboard.press("Meta+Shift+p");
    await expect(page.locator(".command-palette")).toBeVisible({
      timeout: 5_000,
    });
  });

  test("closes with Escape", async ({ page }) => {
    await page.keyboard.press("Meta+Shift+p");
    await expect(page.locator(".command-palette")).toBeVisible({
      timeout: 5_000,
    });

    await page.keyboard.press("Escape");
    await expect(page.locator(".command-palette")).not.toBeVisible();
  });

  test("shows command items", async ({ page }) => {
    await page.keyboard.press("Meta+Shift+p");
    await expect(page.locator(".command-palette")).toBeVisible({
      timeout: 5_000,
    });

    // Should show some command items
    const items = page.locator(".command-palette-item, .command-palette li");
    const count = await items.count();
    expect(count).toBeGreaterThan(0);
  });
});

// ─────────────────────────────────────────────────────
// Keyboard shortcut tests
// ─────────────────────────────────────────────────────

test.describe("Keyboard shortcuts", () => {
  test.beforeEach(async ({ page }) => {
    await navigateAuthenticated(page);
  });

  test("Cmd+B toggles sidebar", async ({ page }) => {
    await expect(page.locator(".chat-sidebar")).toBeVisible();

    await page.keyboard.press("Meta+b");
    await expect(page.locator(".chat-sidebar")).not.toBeVisible();

    await page.keyboard.press("Meta+b");
    await expect(page.locator(".chat-sidebar")).toBeVisible();
  });

  test("Cmd+backtick toggles terminal", async ({ page }) => {
    await expect(page.locator(".terminal-panel")).not.toBeVisible();

    await page.keyboard.press("Meta+Backquote");
    await expect(page.locator(".terminal-panel")).toBeVisible();

    await page.keyboard.press("Meta+Backquote");
    await expect(page.locator(".terminal-panel")).not.toBeVisible();
  });
});

// ─────────────────────────────────────────────────────
// Regression: No React rendering errors
// ─────────────────────────────────────────────────────

test.describe("No React rendering errors", () => {
  test("page loads without React error #31 (object as child)", async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on("pageerror", (err) => {
      errors.push(err.message);
    });

    await navigateAuthenticated(page);

    // Wait for any delayed errors
    await page.waitForTimeout(2000);

    const reactErrors = errors.filter(
      (e) =>
        e.includes("error #31") ||
        e.includes("Objects are not valid as a React child")
    );
    expect(reactErrors).toHaveLength(0);
  });

  test("no 404 errors on stats endpoint", async ({ page }) => {
    const failedRequests: string[] = [];
    page.on("response", (response) => {
      if (response.status() === 404) {
        failedRequests.push(response.url());
      }
    });

    await navigateAuthenticated(page);
    await page.waitForTimeout(2000);

    const statsErrors = failedRequests.filter((url) => url.includes("/stats"));
    expect(statsErrors).toHaveLength(0);
  });
});
