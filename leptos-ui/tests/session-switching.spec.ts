/**
 * Session switching tests.
 */
import { test, expect } from "@playwright/test";
import {
  setupMockAPI,
  SESSION_ID,
  SESSION_ID_2,
  MOCK_APP_STATE,
  MOCK_MESSAGES,
  MOCK_STATS,
  MOCK_COMMANDS,
  MOCK_PROVIDERS,
  MOCK_THEME,
} from "./helpers";

const SESSION_2_MESSAGES = [
  {
    info: {
      role: "user",
      messageID: "msg_201",
      id: "msg_201",
      time: 1700000600,
    },
    parts: [{ type: "text", id: "part_201", messageID: "msg_201", text: "What is the capital of France?" }],
    metadata: null,
  },
  {
    info: {
      role: "assistant",
      messageID: "msg_202",
      id: "msg_202",
      time: 1700000700,
      model: "claude-sonnet-4-20250514",
      modelID: "claude-sonnet-4-20250514",
      providerID: "anthropic",
      cost: 0.001,
      tokens: {
        input: 10,
        output: 15,
        reasoning: 0,
        cache: { read: 0, write: 0 },
      },
    },
    parts: [
      { type: "text", id: "part_202", messageID: "msg_202", text: "The capital of France is Paris." },
    ],
    metadata: {
      cost: 0.001,
      tokens: { input: 10, output: 15 },
    },
  },
];

async function setupSessionSwitching(page: import("@playwright/test").Page) {
  let currentActive = SESSION_ID;

  await setupMockAPI(page);

  // Override state to return dynamic active session
  await page.route("**/api/state", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        ...MOCK_APP_STATE,
        projects: [
          {
            ...MOCK_APP_STATE.projects[0],
            active_session: currentActive,
          },
        ],
      }),
    })
  );

  // Different messages for session 2
  await page.route(`**/api/session/${SESSION_ID_2}/messages*`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ messages: SESSION_2_MESSAGES, has_more: false, total: SESSION_2_MESSAGES.length }),
    })
  );

  // Track session selection
  await page.route("**/api/session/select", (route) => {
    const body = route.request().postDataJSON();
    if (body?.session_id) {
      currentActive = body.session_id;
    }
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    });
  });

  await page.goto("/ui");
  await page.waitForSelector(".chat-layout", { timeout: 15_000 });
}

test.describe("Session switching", () => {
  test.beforeEach(async ({ page }) => {
    await setupSessionSwitching(page);
  });

  test("shows initial session messages", async ({ page }) => {
    await expect(page.getByText("Hello, how are you?")).toBeVisible();
  });

  test("sidebar shows both sessions", async ({ page }) => {
    await expect(page.getByText("Test Session")).toBeVisible();
    await expect(page.getByText("Another Session")).toBeVisible();
  });

  test("clicking a session sends select request", async ({ page }) => {
    const reqPromise = page.waitForRequest(
      (req) =>
        req.url().includes("/api/session/select") &&
        req.method() === "POST",
      { timeout: 5_000 }
    );

    await page.getByText("Another Session").click();
    const req = await reqPromise;
    const body = req.postDataJSON();
    expect(body.session_id).toBe(SESSION_ID_2);
  });

  test("active session has active class", async ({ page }) => {
    const activeSessions = page.locator(".sb-session.active");
    await expect(activeSessions).toHaveCount(1);
  });
});
