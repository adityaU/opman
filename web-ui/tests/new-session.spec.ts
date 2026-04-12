/**
 * Test: New session creation via the "+" sidebar button.
 *
 * Validates that clicking "+" properly creates a new session and
 * re-enables the input textarea and model picker.
 */

import { test, expect, Page } from "@playwright/test";
import {
  SESSION_ID,
  SESSION_ID_2,
  MOCK_APP_STATE,
  MOCK_MESSAGES,
  MOCK_STATS,
  MOCK_COMMANDS,
  MOCK_PROVIDERS,
  MOCK_THEME,
} from "./helpers";

// ── Helpers ───────────────────────────────────────────

/**
 * Set up dynamic mock API routes.
 * Returns controllers to swap responses at runtime.
 */
async function setupDynamicMocks(page: Page) {
  let currentAppState: any = MOCK_APP_STATE;

  // ── Catch-all: prevent any unmocked /api/* request from hitting the
  //    real backend (which isn't running) and returning 401. ──
  //    In Playwright, the LAST registered route for a URL wins, so this
  //    catch-all is registered first and will be overridden by specific
  //    routes below.
  //    NOTE: Playwright checks routes in REVERSE registration order.
  //    So we register the catch-all first (lowest priority).
  // Catch-all: use pathname check to avoid intercepting Vite source-file
  // requests like /src/api/client.ts which match the old "**/api/**" glob.
  await page.route(/\/api\//, (route) => {
    const url = new URL(route.request().url());
    if (url.pathname.startsWith("/api/")) {
      return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({}) });
    }
    return route.continue();
  });

  // Intercept specific API requests (these override the catch-all)
  await page.route("**/api/auth/verify", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  await page.route("**/api/state", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(currentAppState),
    })
  );

  await page.route(`**/api/session/*/messages*`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ messages: [], total: 0 }),
    })
  );

  await page.route("**/api/session/*/stats", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_STATS),
    })
  );

  await page.route("**/api/commands", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_COMMANDS),
    })
  );

  await page.route("**/api/providers", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_PROVIDERS),
    })
  );

  await page.route("**/api/theme", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_THEME),
    })
  );

  // SSE endpoints
  await page.route("**/api/events*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "text/event-stream",
      headers: { "Cache-Control": "no-cache" },
      body: "data: {}\n\n",
    })
  );

  await page.route("**/api/session/events*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "text/event-stream",
      headers: { "Cache-Control": "no-cache" },
      body: "data: {}\n\n",
    })
  );

  // POST endpoints
  await page.route("**/api/session/*/message", (route) => {
    if (route.request().method() === "POST") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ ok: true }),
      });
    }
    return route.continue();
  });

  await page.route("**/api/session/*/abort", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  await page.route("**/api/session/*/command", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  await page.route("**/api/session/select", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  await page.route("**/api/session/new", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  await page.route("**/api/project/switch", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  await page.route("**/api/session/*/todos", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify([]),
    })
  );

  await page.route("**/api/session/*/permission", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  await page.route("**/api/session/*/question", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  // Register/heartbeat/presence
  await page.route("**/api/presence", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ clients: [] }) })
  );

  // OpenSpec assistant endpoints
  await page.route("**/api/memory", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ memory: [] }) })
  );

  await page.route("**/api/autonomy", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ mode: "observe", updated_at: new Date().toISOString() }),
    })
  );

  await page.route("**/api/routines", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ routines: [], runs: [] }) })
  );

  await page.route("**/api/missions", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ missions: [] }) })
  );

  await page.route("**/api/delegation", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ items: [] }) })
  );

  await page.route("**/api/workspaces", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ workspaces: [] }) })
  );

  return {
    setAppState(state: any) {
      currentAppState = state;
    },
  };
}

/**
 * Install EventSource spy to dispatch SSE events from tests.
 */
async function installEventSourceSpy(page: Page) {
  await page.addInitScript(() => {
    (window as any).__eventSources = [];
    const OrigES = window.EventSource;
    const PatchedES = function (
      this: EventSource,
      url: string | URL,
      opts?: EventSourceInit
    ) {
      const es = new OrigES(url, opts);
      (window as any).__eventSources.push(es);
      return es;
    } as unknown as typeof EventSource;
    PatchedES.prototype = OrigES.prototype;
    PatchedES.CONNECTING = OrigES.CONNECTING;
    PatchedES.OPEN = OrigES.OPEN;
    PatchedES.CLOSED = OrigES.CLOSED;
    window.EventSource = PatchedES;
  });
}

async function dispatchAppSSE(page: Page, eventType: string, data: string) {
  await page.evaluate(
    ({ eventType, data }) => {
      const event = new MessageEvent(eventType, { data });
      const sources = (window as any).__eventSources || [];
      for (const es of sources) {
        if (
          es.url &&
          es.url.includes("/api/events") &&
          !es.url.includes("/api/session/events")
        ) {
          es.dispatchEvent(event);
        }
      }
    },
    { eventType, data }
  );
}

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

    // ── Step 4: Textarea and model chip should become disabled ──
    await expect(textarea).toBeDisabled({ timeout: 5_000 });
    await expect(modelChip).toBeDisabled({ timeout: 3_000 });

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
    await expect(page.locator(".sb-session-title", { hasText: "New Session" })).toBeVisible({ timeout: 3_000 });
  });
});
