/**
 * Test: New session creation via the "+" sidebar button.
 *
 * Validates that clicking "+" properly creates a new session and
 * re-enables the input textarea and model picker.
 */

import { test, expect } from "@playwright/test";
import { MOCK_APP_STATE } from "./helpers";
import { setupDynamicMocks } from "./newSessionMocks";
import { installEventSourceSpy, dispatchAppSSE } from "./newSessionFixtures";

// ── Tests ─────────────────────────────────────────────

test.describe("New session creation", () => {
  test("diagnostic: can navigate and see chat layout", async ({ page }) => {
    // Log all requests to see what's happening
    const interceptedUrls: string[] = [];
    page.on("request", (req) => {
      interceptedUrls.push(`${req.method()} ${req.url()}`);
    });
    page.on("response", (resp) => {
      if (resp.url().includes("/api/")) {
        console.log(`Response: ${resp.status()} ${resp.url()}`);
      }
    });

    await installEventSourceSpy(page);
    const mocks = await setupDynamicMocks(page);

    // Navigate
    await page.goto("/");

    // Inject auth token
    await page.evaluate(() => {
      sessionStorage.setItem("opman_token", "mock-jwt-token");
    });

    // Reload to pick up the token
    await page.reload({ waitUntil: "domcontentloaded" });

    // Wait a bit for API calls to settle
    await page.waitForTimeout(3000);

    // Take screenshot for debugging
    await page.screenshot({ path: "test-results/diag-after-reload.png" });

    // Log all intercepted URLs
    const apiCalls = interceptedUrls.filter(u => u.includes("/api/"));
    console.log("API calls made:", JSON.stringify(apiCalls, null, 2));

    // Check what's visible
    const body = await page.locator("body").innerHTML();
    console.log("Body HTML (first 1000 chars):", body.substring(0, 1000));

    // Try to find chat-layout
    const chatLayout = page.locator(".chat-layout");
    await expect(chatLayout).toBeVisible({ timeout: 15_000 });
  });

  test("clicking + button creates session and re-enables input", async ({ page }) => {
    await installEventSourceSpy(page);
    const mocks = await setupDynamicMocks(page);

    await page.goto("/");
    await page.evaluate(() => {
      sessionStorage.setItem("opman_token", "mock-jwt-token");
    });
    await page.reload({ waitUntil: "networkidle" });
    await page.waitForSelector(".chat-layout", { timeout: 15_000 });

    // Wait for SSE to settle
    await page.waitForTimeout(500);

    // ── Step 1: Verify initial state — textarea and model chip are enabled ──
    const textarea = page.locator(".prompt-textarea");
    await expect(textarea).toBeVisible();
    await expect(textarea).not.toBeDisabled();

    const modelChip = page.locator(".prompt-chip").first();
    await expect(modelChip).not.toBeDisabled();

    // ── Step 2: Prepare state change — when new session is called,
    //    the backend clears active_session. The next refreshState()
    //    call will return this null-active-session state. ──
    mocks.setAppState({
      ...MOCK_APP_STATE,
      projects: [
        {
          ...MOCK_APP_STATE.projects[0],
          active_session: null,
        },
      ],
    });

    // ── Step 3: Click the "+" new session button ──
    const newBtn = page.locator(".sb-new-btn");
    await expect(newBtn).toBeVisible();
    await newBtn.click();

    // ── Step 4: Lazy creation keeps input and selectors usable ──
    await expect(textarea).not.toBeDisabled({ timeout: 5_000 });
    await expect(modelChip).not.toBeDisabled({ timeout: 3_000 });

    // ── Step 5: Simulate backend creating the session ──
    //    After the headless PTY triggers session creation, the backend
    //    auto-activates the new session and emits state_changed.
    const NEW_SESSION_ID = "ses_new_from_plus";
    mocks.setAppState({
      ...MOCK_APP_STATE,
      projects: [
        {
          ...MOCK_APP_STATE.projects[0],
          active_session: NEW_SESSION_ID,
          sessions: [
            ...MOCK_APP_STATE.projects[0].sessions,
            {
              id: NEW_SESSION_ID,
              title: "New Session",
              parentID: "",
              directory: "/home/user/my-project",
              time: { created: 1700005000, updated: 1700005000 },
            },
          ],
        },
      ],
    });

    // Dispatch state_changed SSE event
    await dispatchAppSSE(page, "state_changed", "");

    // ── Step 6: Textarea and model chip should be re-enabled ──
    await expect(textarea).not.toBeDisabled({ timeout: 5_000 });
    await expect(modelChip).not.toBeDisabled({ timeout: 3_000 });

    // ── Step 7: Verify user can type ──
    await textarea.fill("Hello from new session!");
    await expect(textarea).toHaveValue("Hello from new session!");

    // ── Step 8: New session appears in sidebar ──
    await expect(page.locator(".sb-session-title", { hasText: "New Session" }).first()).toBeVisible({ timeout: 3_000 });
  });

  test("first send transitions from new URL to a real session without losing the prompt", async ({ page }) => {
    await installEventSourceSpy(page);
    await setupDynamicMocks(page, { messageDelayMs: 250 });
    await page.goto("/?new=1");
    await page.evaluate(() => sessionStorage.setItem("opman_token", "mock-jwt-token"));
    await page.reload({ waitUntil: "networkidle" });
    await page.waitForSelector(".chat-layout", { timeout: 15_000 });

    await expect(page).toHaveURL(/new=1/);
    await expect(page.locator(".message-timeline-welcome")).toBeVisible();

    const marker = "First prompt survives lazy session creation";
    const messageRequest = page.waitForRequest((request) => (
      request.method() === "POST" && new URL(request.url()).pathname.endsWith("/message")
    ));
    await page.locator(".prompt-textarea").fill(marker);
    await page.locator(".prompt-send-btn").click();

    // The optimistic turn and progress state must appear before the create/send
    // round-trip finishes; otherwise the new-session screen looks unresponsive.
    await expect(page.locator(".pending-reply")).toBeVisible({ timeout: 10_000 });
    await expect(page.locator(".message-turn-user").filter({ hasText: marker })).toBeVisible({ timeout: 10_000 });
    await expect(page.locator(".message-timeline-welcome")).toHaveCount(0);
    await messageRequest;
    await expect(page).toHaveURL(/session=ses_new_from_first_send_1/);
  });

  test("selecting an existing session exits new-session mode cleanly", async ({ page }) => {
    await installEventSourceSpy(page);
    await setupDynamicMocks(page);
    await page.goto("/?new=1");
    await page.evaluate(() => sessionStorage.setItem("opman_token", "mock-jwt-token"));
    await page.reload({ waitUntil: "networkidle" });
    await page.waitForSelector(".chat-layout", { timeout: 15_000 });

    await page.locator(".sb-session-title", { hasText: "Test Session" }).first().click();
    await expect(page).toHaveURL(/session=ses_test_session_001/);
    await expect(page).not.toHaveURL(/new=1/);
    await expect(page.locator(".prompt-textarea")).not.toBeDisabled();
  });
});
