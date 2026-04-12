/**
 * Tests for session switching — clicking a different session in the sidebar.
 *
 * Validates:
 *  - Clicking another session triggers select API call
 *  - Active session highlight changes
 *  - Messages are re-fetched for the new session
 *  - Switching back preserves expected behavior
 */

import { test, expect, Page } from "@playwright/test";
import {
  SESSION_ID,
  SESSION_ID_2,
  MOCK_APP_STATE,
  MOCK_MESSAGES,
  MOCK_STATS,
  MOCK_PROVIDERS,
  MOCK_COMMANDS,
  MOCK_THEME,
} from "./helpers";

// Second session has its own messages
const SESSION_2_MESSAGES = [
  {
    info: {
      role: "user",
      messageID: "msg_201",
      time: 1700000600,
    },
    parts: [{ type: "text", text: "What is the capital of France?" }],
    metadata: {},
  },
  {
    info: {
      role: "assistant",
      messageID: "msg_202",
      time: 1700000700,
      model: { modelID: "claude-sonnet-4-20250514", providerID: "anthropic" },
    },
    parts: [
      {
        type: "text",
        text: "The capital of France is Paris.",
      },
    ],
    metadata: {
      cost: 0.001,
      tokens: { input: 10, output: 15, reasoning: 0, cache_read: 0, cache_write: 0 },
    },
  },
];

/**
 * Dynamic mock setup that tracks which session is active and serves
 * different messages per session.
 */
async function setupDynamicSessionMocks(page: Page) {
  let activeSession = SESSION_ID;

  // Catch-all: use pathname check to avoid intercepting Vite source-file
  // requests like /src/api/client.ts which match the old "**/api/**" glob.
  await page.route(/\/api\//, (route) => {
    const url = new URL(route.request().url());
    if (url.pathname.startsWith("/api/")) {
      return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({}) });
    }
    return route.continue();
  });

  await page.route("**/api/auth/verify", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ok: true }) })
  );

  await page.route("**/api/auth/login", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ token: "mock-jwt-token" }),
    })
  );

  await page.route("**/api/state", (route) => {
    const state = {
      ...MOCK_APP_STATE,
      projects: [
        {
          ...MOCK_APP_STATE.projects[0],
          active_session: activeSession,
        },
      ],
    };
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(state),
    });
  });

  // Session 1 messages
  await page.route(`**/api/session/${SESSION_ID}/messages*`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ messages: MOCK_MESSAGES, total: MOCK_MESSAGES.length }),
    })
  );

  // Session 2 messages
  await page.route(`**/api/session/${SESSION_ID_2}/messages*`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ messages: SESSION_2_MESSAGES, total: SESSION_2_MESSAGES.length }),
    })
  );

  await page.route("**/api/session/*/stats", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MOCK_STATS) })
  );

  await page.route("**/api/commands", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MOCK_COMMANDS) })
  );

  await page.route("**/api/providers", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MOCK_PROVIDERS) })
  );

  await page.route("**/api/theme", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MOCK_THEME) })
  );

  await page.route("**/api/events*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "text/event-stream",
      body: "data: {}\n\n",
    })
  );

  await page.route("**/api/session/events*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "text/event-stream",
      body: "data: {}\n\n",
    })
  );

  // Track session select calls and update active session
  await page.route("**/api/session/select", (route) => {
    const body = route.request().postDataJSON();
    if (body?.session_id) {
      activeSession = body.session_id;
    }
    return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ok: true }) });
  });

  await page.route("**/api/session/new", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ok: true }) })
  );

  await page.route("**/api/project/switch", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ok: true }) })
  );

  await page.route("**/api/session/*/todos", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify([]) })
  );

  await page.route("**/api/session/*/permission", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ok: true }) })
  );

  await page.route("**/api/session/*/question", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ok: true }) })
  );

  await page.route("**/api/presence", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ clients: [] }) })
  );

  await page.route("**/api/agents", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify([{ id: "default", name: "Default Agent", system_prompt: "" }]),
    })
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

  // POST message endpoint
  await page.route("**/api/session/*/message", (route) => {
    if (route.request().method() === "POST") {
      return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ok: true }) });
    }
    return route.continue();
  });

  return {
    getActiveSession: () => activeSession,
  };
}

async function navigateWithDynamicMocks(page: Page) {
  const mocks = await setupDynamicSessionMocks(page);
  await page.goto("/");
  await page.evaluate(() => {
    sessionStorage.setItem("opman_token", "mock-jwt-token");
  });
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });
  return mocks;
}

test.describe("Session switching", () => {
  test("sidebar shows both sessions", async ({ page }) => {
    await navigateWithDynamicMocks(page);
    const groups = page.locator(".sb-session-group");
    await expect(groups).toHaveCount(2);
  });

  test("initial active session shows its messages", async ({ page }) => {
    await navigateWithDynamicMocks(page);
    // Session 1 messages should be visible
    await expect(page.getByText("Hello, how are you?")).toBeVisible();
    await expect(page.getByText("I'm doing well!")).toBeVisible();
  });

  test("clicking another session triggers select API call", async ({ page }) => {
    const mocks = await navigateWithDynamicMocks(page);

    // Verify initial state
    expect(mocks.getActiveSession()).toBe(SESSION_ID);

    // Find and click on "Another Session" title
    const secondSession = page.getByText("Another Session");
    await expect(secondSession).toBeVisible();

    // Intercept select call
    const selectPromise = page.waitForRequest(
      (req) => req.url().includes("/api/session/select") && req.method() === "POST"
    );

    await secondSession.click();

    // Verify select API was called
    const selectReq = await selectPromise;
    const body = selectReq.postDataJSON();
    expect(body.session_id).toBe(SESSION_ID_2);
  });

  test("clicking another session loads its messages", async ({ page }) => {
    await navigateWithDynamicMocks(page);

    // Initially session 1 messages
    await expect(page.getByText("Hello, how are you?")).toBeVisible();

    // Click session 2
    await page.getByText("Another Session").click();

    // Wait for session 2 messages to appear
    await expect(page.getByText("What is the capital of France?")).toBeVisible({ timeout: 5000 });
    await expect(page.getByText("The capital of France is Paris.")).toBeVisible({ timeout: 5000 });
  });

  test("active session gets highlighted class", async ({ page }) => {
    await navigateWithDynamicMocks(page);

    // Session 1 should be active initially — "active" class is on .sb-session, not .sb-session-group
    const activeSessions = page.locator(".sb-session.active");
    await expect(activeSessions).toHaveCount(1);
    await expect(activeSessions.first()).toContainText("Test Session");
  });

  test("switching back to original session restores its messages", async ({ page }) => {
    await navigateWithDynamicMocks(page);

    // Start on session 1
    await expect(page.getByText("Hello, how are you?")).toBeVisible();

    // Switch to session 2
    await page.getByText("Another Session").click();
    await expect(page.getByText("What is the capital of France?")).toBeVisible({ timeout: 5000 });

    // Switch back to session 1
    await page.getByText("Test Session").click();
    await expect(page.getByText("Hello, how are you?")).toBeVisible({ timeout: 5000 });
  });
});
