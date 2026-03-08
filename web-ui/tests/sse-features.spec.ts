/**
 * Tests for SSE (Server-Sent Events) features.
 *
 * These tests validate the fixes we made:
 *   ISSUE 1: broadcast channel (ensures no events are dropped)
 *   ISSUE 2: session.status nested object format
 *   ISSUE 3: duplicate SSE listener prevention (backend-only, not testable here)
 *   ISSUE 4: message.updated filtered by session ID
 *
 * Strategy: We monkey-patch EventSource via addInitScript (survives reloads),
 * intercept the SSE endpoints, then dispatch events from the test to verify
 * the UI reacts correctly.
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

// ── Mock data variants ────────────────────────────────

/** Updated mock messages with a new message appended */
const UPDATED_MESSAGES = [
  ...MOCK_MESSAGES,
  {
    info: {
      role: "assistant",
      messageID: "msg_004",
      time: 1700000400,
      model: { modelID: "claude-sonnet-4-20250514", providerID: "anthropic" },
    },
    parts: [
      {
        type: "text",
        text: "This is a new streamed message from the assistant!",
      },
    ],
    metadata: {
      cost: 0.001,
      tokens: { input: 10, output: 20, reasoning: 0, cache_read: 0, cache_write: 0 },
    },
  },
];

/** Updated stats */
const UPDATED_STATS = {
  cost: 0.025,
  input_tokens: 500,
  output_tokens: 1200,
  reasoning_tokens: 100,
  cache_read: 50,
  cache_write: 20,
};

/** A different theme to test theme_changed */
const NEW_THEME = {
  primary: "#e11d48",
  secondary: "#06b6d4",
  accent: "#a855f7",
  background: "#1a1a2e",
  background_panel: "#16213e",
  background_element: "#0f3460",
  text: "#e2e8f0",
  text_muted: "#64748b",
  border: "#334155",
  border_active: "#e11d48",
  border_subtle: "#1e293b",
  error: "#f43f5e",
  warning: "#eab308",
  success: "#22c55e",
  info: "#0ea5e9",
};

// ── Helpers ───────────────────────────────────────────

/**
 * Set up mock API routes with dynamic response capability.
 * Returns controllers that let tests swap responses at runtime.
 */
async function setupDynamicMockAPI(page: Page) {
  let currentAppState = MOCK_APP_STATE;
  let currentMessages = MOCK_MESSAGES;
  let currentStats = MOCK_STATS;

  // Catch-all: prevent any unmocked /api/* request from hitting the real
  // backend. Registered first = lowest priority in Playwright.
  await page.route("**/api/**", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({}) })
  );

  await page.route("**/api/auth/verify", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  await page.route("**/api/auth/login", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ token: "mock-jwt-token" }),
    })
  );

  await page.route("**/api/state", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(currentAppState),
    })
  );

  await page.route(`**/api/session/${SESSION_ID}/messages*`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ messages: currentMessages, total: currentMessages.length }),
    })
  );

  await page.route(`**/api/session/${SESSION_ID_2}/messages*`, (route) =>
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
      body: JSON.stringify(currentStats),
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

  // SSE endpoints — return an empty SSE stream
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
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ok: true }) })
  );

  await page.route("**/api/session/*/command", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ok: true }) })
  );

  await page.route("**/api/session/select", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ok: true }) })
  );

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

  return {
    setAppState(state: typeof MOCK_APP_STATE) {
      currentAppState = state;
    },
    setMessages(msgs: typeof MOCK_MESSAGES) {
      currentMessages = msgs;
    },
    setStats(stats: typeof MOCK_STATS) {
      currentStats = stats;
    },
  };
}

/**
 * Install the EventSource spy via addInitScript so it survives page reloads.
 * This MUST be called before the first page.goto().
 */
async function installEventSourceSpy(page: Page) {
  await page.addInitScript(() => {
    // Capture all EventSource instances for later event injection
    (window as any).__eventSources = [];
    const OrigES = window.EventSource;
    const PatchedES = function (this: EventSource, url: string | URL, opts?: EventSourceInit) {
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

/**
 * Full setup: install spy, set up mocks, navigate, authenticate, wait for layout.
 */
async function setupAndNavigate(page: Page) {
  await installEventSourceSpy(page);
  const mocks = await setupDynamicMockAPI(page);

  await page.goto("/");
  await page.evaluate(() => {
    sessionStorage.setItem("opman_token", "mock-jwt-token");
  });
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });

  // Brief wait for SSE connections to be established
  await page.waitForTimeout(300);

  return mocks;
}

/**
 * Dispatch an app-level SSE event (event type = the SSE event name).
 */
async function dispatchAppSSE(page: Page, eventType: string, data: string) {
  await page.evaluate(
    ({ eventType, data }) => {
      const event = new MessageEvent(eventType, { data });
      const sources = (window as any).__eventSources || [];
      for (const es of sources) {
        if (es.url && es.url.includes("/api/events") && !es.url.includes("/api/session/events")) {
          es.dispatchEvent(event);
        }
      }
    },
    { eventType, data }
  );
}

/**
 * Dispatch a session-level SSE event (event type = "opencode", data = JSON).
 */
async function dispatchSessionSSE(
  page: Page,
  openCodeEvent: { type: string; properties: Record<string, unknown> }
) {
  await page.evaluate(
    ({ eventData }) => {
      const event = new MessageEvent("opencode", {
        data: JSON.stringify(eventData),
      });
      const sources = (window as any).__eventSources || [];
      for (const es of sources) {
        if (es.url && es.url.includes("/api/session/events")) {
          es.dispatchEvent(event);
        }
      }
    },
    { eventData: openCodeEvent }
  );
}

// ── Tests: App-level SSE events ───────────────────────

test.describe("SSE: App-level events (/api/events)", () => {
  test("state_changed event triggers UI refresh with new state", async ({ page }) => {
    const mocks = await setupAndNavigate(page);

    // Initially should show 2 sessions
    await expect(page.getByText("Test Session")).toBeVisible();
    await expect(page.locator(".sb-session-group")).toHaveCount(2);

    // Update mock state to add a third session
    mocks.setAppState({
      ...MOCK_APP_STATE,
      projects: [
        {
          ...MOCK_APP_STATE.projects[0],
          sessions: [
            ...MOCK_APP_STATE.projects[0].sessions,
            {
              id: "ses_new_session_003",
              title: "Brand New Session",
              parentID: "",
              directory: "/home/user/my-project",
              time: { created: 1700002000, updated: 1700002000 },
            },
          ],
        },
      ],
    });

    // Dispatch state_changed event
    await dispatchAppSSE(page, "state_changed", "");

    // New session should appear
    await expect(page.getByText("Brand New Session")).toBeVisible({ timeout: 5_000 });
  });

  test("session_busy event updates status bar to busy", async ({ page }) => {
    await setupAndNavigate(page);

    // Status should initially be "ready"
    await expect(page.locator(".status-bar-status")).toHaveText("ready");

    // Dispatch session_busy for the active session
    await dispatchAppSSE(page, "session_busy", SESSION_ID);

    // Status should change to "busy"
    await expect(page.locator(".status-bar-status")).toHaveText("busy", { timeout: 3_000 });

    // The stop button should appear (pulsing abort button replaces thinking indicator)
    await expect(page.locator(".prompt-abort-btn")).toBeVisible({ timeout: 3_000 });
  });

  test("session_idle event restores status bar to ready", async ({ page }) => {
    await setupAndNavigate(page);

    // First set busy
    await dispatchAppSSE(page, "session_busy", SESSION_ID);
    await expect(page.locator(".status-bar-status")).toHaveText("busy", { timeout: 3_000 });

    // Then set idle
    await dispatchAppSSE(page, "session_idle", SESSION_ID);
    await expect(page.locator(".status-bar-status")).toHaveText("ready", { timeout: 3_000 });

    // Thinking indicator should disappear
    await expect(page.locator(".message-thinking")).not.toBeVisible();
  });

  test("session_busy for different session does NOT change active session status", async ({ page }) => {
    await setupAndNavigate(page);

    await expect(page.locator(".status-bar-status")).toHaveText("ready");

    // Dispatch session_busy for a DIFFERENT session
    await dispatchAppSSE(page, "session_busy", SESSION_ID_2);

    await page.waitForTimeout(500);
    await expect(page.locator(".status-bar-status")).toHaveText("ready");
  });

  test("stats_updated event updates cost display", async ({ page }) => {
    await setupAndNavigate(page);

    // Wait for initial stats to load
    await expect(page.locator(".status-bar-cost")).toBeVisible();

    // Dispatch stats_updated with new stats
    await dispatchAppSSE(page, "stats_updated", JSON.stringify(UPDATED_STATS));

    // Check that cost updated
    await expect(page.locator(".status-bar-cost")).toContainText("0.025", { timeout: 3_000 });
  });

  test("theme_changed event applies new CSS custom properties", async ({ page }) => {
    await setupAndNavigate(page);

    // Verify original primary color
    const originalPrimary = await page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue("--color-primary").trim()
    );
    expect(originalPrimary).toBe("#7c3aed");

    // Dispatch theme_changed
    await dispatchAppSSE(page, "theme_changed", JSON.stringify(NEW_THEME));

    // Verify CSS variable changed
    await page.waitForTimeout(300);
    const newPrimary = await page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue("--color-primary").trim()
    );
    expect(newPrimary).toBe("#e11d48");

    const newBg = await page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue("--color-bg").trim()
    );
    expect(newBg).toBe("#1a1a2e");
  });
});

// ── Tests: Session-level SSE events ───────────────────

test.describe("SSE: Session-level events (/api/session/events)", () => {
  test("message.updated triggers message list refresh", async ({ page }) => {
    const mocks = await setupAndNavigate(page);

    // Initially should see original messages
    await expect(page.getByText("Hello, how are you?")).toBeVisible();
    await expect(page.getByText("This is a new streamed message")).not.toBeVisible();

    // Dispatch message.updated for the active session — creates the message entry
    await dispatchSessionSSE(page, {
      type: "message.updated",
      properties: {
        info: {
          sessionID: SESSION_ID,
          messageID: "msg_004",
          role: "assistant",
          time: 1700000400,
          model: { modelID: "claude-sonnet-4-20250514", providerID: "anthropic" },
        },
      },
    });

    // Dispatch message.part.updated — adds the text part to the message
    await dispatchSessionSSE(page, {
      type: "message.part.updated",
      properties: {
        part: {
          sessionID: SESSION_ID,
          messageID: "msg_004",
          id: "part_004_0",
          type: "text",
          text: "This is a new streamed message from the assistant!",
        },
      },
    });

    // New message should appear
    await expect(
      page.getByText("This is a new streamed message from the assistant!")
    ).toBeVisible({ timeout: 5_000 });
  });

  test("message.updated for different session does NOT refresh messages (ISSUE 4 fix)", async ({ page }) => {
    const mocks = await setupAndNavigate(page);

    await expect(page.getByText("Hello, how are you?")).toBeVisible();

    // Update mock to return different messages
    mocks.setMessages(UPDATED_MESSAGES as any);

    // Dispatch for a DIFFERENT session
    await dispatchSessionSSE(page, {
      type: "message.updated",
      properties: {
        info: { sessionID: "ses_completely_different_session", messageID: "msg_999" },
      },
    });

    await page.waitForTimeout(1000);

    // New message should NOT appear
    await expect(page.getByText("This is a new streamed message")).not.toBeVisible();
  });

  test("session.status with nested object format updates status (ISSUE 2 fix)", async ({ page }) => {
    await setupAndNavigate(page);

    await expect(page.locator(".status-bar-status")).toHaveText("ready");

    // Dispatch session.status with NESTED object format (actual upstream format)
    await dispatchSessionSSE(page, {
      type: "session.status",
      properties: {
        sessionID: SESSION_ID,
        status: { type: "busy" },
      },
    });

    await expect(page.locator(".status-bar-status")).toHaveText("busy", { timeout: 3_000 });
  });

  test("session.status with string format also works", async ({ page }) => {
    await setupAndNavigate(page);

    // Dispatch with plain string status
    await dispatchSessionSSE(page, {
      type: "session.status",
      properties: { sessionID: SESSION_ID, status: "busy" },
    });

    await expect(page.locator(".status-bar-status")).toHaveText("busy", { timeout: 3_000 });

    // Now send idle
    await dispatchSessionSSE(page, {
      type: "session.status",
      properties: { sessionID: SESSION_ID, status: "idle" },
    });

    await expect(page.locator(".status-bar-status")).toHaveText("ready", { timeout: 3_000 });
  });

  test("session.status with 'retry' type shows as busy", async ({ page }) => {
    await setupAndNavigate(page);

    await dispatchSessionSSE(page, {
      type: "session.status",
      properties: { sessionID: SESSION_ID, status: { type: "retry" } },
    });

    await expect(page.locator(".status-bar-status")).toHaveText("busy", { timeout: 3_000 });
  });

  test("session.status for different session does NOT affect active status", async ({ page }) => {
    await setupAndNavigate(page);

    await expect(page.locator(".status-bar-status")).toHaveText("ready");

    await dispatchSessionSSE(page, {
      type: "session.status",
      properties: { sessionID: SESSION_ID_2, status: { type: "busy" } },
    });

    await page.waitForTimeout(500);
    await expect(page.locator(".status-bar-status")).toHaveText("ready");
  });

  test("permission.asked shows permission dialog with tool name and actions", async ({ page }) => {
    await setupAndNavigate(page);

    // Initially no permission dock visible
    await expect(page.locator(".permission-card")).not.toBeVisible();

    // Dispatch permission.asked
    await dispatchSessionSSE(page, {
      type: "permission.asked",
      properties: {
        id: "perm_001",
        sessionID: SESSION_ID,
        toolName: "write_file",
        description: "Write to /src/main.ts",
        args: { path: "/src/main.ts", content: "hello world" },
      },
    });

    // Permission card should appear
    await expect(page.locator(".permission-card")).toBeVisible({ timeout: 3_000 });

    // Should show tool name
    await expect(page.locator(".permission-tool")).toContainText("write_file");

    // Should show description
    await expect(page.locator(".permission-desc")).toContainText("Write to /src/main.ts");

    // Should have action buttons
    await expect(page.getByRole("button", { name: "Allow Once" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Always Allow" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Reject" })).toBeVisible();
  });

  test("permission.asked for different session does NOT show dialog", async ({ page }) => {
    await setupAndNavigate(page);

    await dispatchSessionSSE(page, {
      type: "permission.asked",
      properties: {
        id: "perm_other",
        sessionID: "ses_other_session",
        toolName: "write_file",
        description: "Should not appear",
      },
    });

    await page.waitForTimeout(500);
    await expect(page.locator(".permission-card")).not.toBeVisible();
  });

  test("question.asked shows question dialog with title and options", async ({ page }) => {
    await setupAndNavigate(page);

    await expect(page.locator(".question-card")).not.toBeVisible();

    // Dispatch question.asked with a select question
    await dispatchSessionSSE(page, {
      type: "question.asked",
      properties: {
        id: "q_001",
        sessionID: SESSION_ID,
        title: "Choose a framework",
        questions: [
          {
            text: "Which framework do you prefer?",
            type: "select",
            options: ["React", "Vue", "Svelte"],
          },
        ],
      },
    });

    // Question card should appear
    await expect(page.locator(".question-card")).toBeVisible({ timeout: 3_000 });

    // Should show title
    await expect(page.locator(".question-title")).toContainText("Choose a framework");

    // Should show question text
    await expect(page.getByText("Which framework do you prefer?")).toBeVisible();

    // Should show options
    await expect(page.locator(".question-option").filter({ hasText: "React" })).toBeVisible();
    await expect(page.locator(".question-option").filter({ hasText: "Vue" })).toBeVisible();
    await expect(page.locator(".question-option").filter({ hasText: "Svelte" })).toBeVisible();

    // Should have Submit button
    await expect(page.getByRole("button", { name: "Submit answers" })).toBeVisible();
  });

  test("question.asked with confirm type shows Yes/No options", async ({ page }) => {
    await setupAndNavigate(page);

    await dispatchSessionSSE(page, {
      type: "question.asked",
      properties: {
        id: "q_confirm",
        sessionID: SESSION_ID,
        title: "Confirm action",
        questions: [
          { text: "Do you want to proceed?", type: "confirm" },
        ],
      },
    });

    await expect(page.locator(".question-card")).toBeVisible({ timeout: 3_000 });

    await expect(page.locator(".question-option").filter({ hasText: "Yes" })).toBeVisible();
    await expect(page.locator(".question-option").filter({ hasText: "No" })).toBeVisible();
  });

  test("question.asked for different session does NOT show dialog", async ({ page }) => {
    await setupAndNavigate(page);

    await dispatchSessionSSE(page, {
      type: "question.asked",
      properties: {
        id: "q_other",
        sessionID: "ses_other_session",
        title: "Should not appear",
        questions: [{ text: "Hidden", type: "text" }],
      },
    });

    await page.waitForTimeout(500);
    await expect(page.locator(".question-card")).not.toBeVisible();
  });

  test("session.created triggers state refresh", async ({ page }) => {
    const mocks = await setupAndNavigate(page);

    await expect(page.locator(".sb-session-group")).toHaveCount(2);

    // Update state to include new session
    mocks.setAppState({
      ...MOCK_APP_STATE,
      projects: [
        {
          ...MOCK_APP_STATE.projects[0],
          sessions: [
            ...MOCK_APP_STATE.projects[0].sessions,
            {
              id: "ses_created",
              title: "Created via event",
              parentID: "",
              directory: "/home/user/my-project",
              time: { created: 1700003000, updated: 1700003000 },
            },
          ],
        },
      ],
    });

    // Dispatch session.created
    await dispatchSessionSSE(page, {
      type: "session.created",
      properties: { sessionID: "ses_created" },
    });

    await expect(page.getByText("Created via event")).toBeVisible({ timeout: 5_000 });
  });
});

// ── Tests: Busy indicator ─────────────────────────────

test.describe("SSE: Busy indicator in prompt input", () => {
  test("shows stop button when busy, hides when idle", async ({ page }) => {
    await setupAndNavigate(page);

    // No stop button initially
    await expect(page.locator(".prompt-abort-btn")).not.toBeVisible();

    // Make busy
    await dispatchAppSSE(page, "session_busy", SESSION_ID);

    // Stop button should appear
    await expect(page.locator(".prompt-abort-btn")).toBeVisible({ timeout: 3_000 });

    // Make idle
    await dispatchAppSSE(page, "session_idle", SESSION_ID);

    // Stop button should disappear
    await expect(page.locator(".prompt-abort-btn")).not.toBeVisible({ timeout: 3_000 });
  });
});

// ── Tests: Rapid events (broadcast channel validation) ─

test.describe("SSE: Multiple rapid events (broadcast channel)", () => {
  test("multiple session_busy/idle toggles settle to correct final state", async ({ page }) => {
    await setupAndNavigate(page);

    // Rapid toggles — final state should be busy
    await dispatchAppSSE(page, "session_busy", SESSION_ID);
    await dispatchAppSSE(page, "session_idle", SESSION_ID);
    await dispatchAppSSE(page, "session_busy", SESSION_ID);

    await expect(page.locator(".status-bar-status")).toHaveText("busy", { timeout: 3_000 });

    // One more idle — final state should be ready
    await dispatchAppSSE(page, "session_idle", SESSION_ID);
    await expect(page.locator(".status-bar-status")).toHaveText("ready", { timeout: 3_000 });
  });

  test("rapid session.status events with nested format all arrive", async ({ page }) => {
    await setupAndNavigate(page);

    // Rapid busy -> idle -> busy
    await dispatchSessionSSE(page, {
      type: "session.status",
      properties: { sessionID: SESSION_ID, status: { type: "busy" } },
    });
    await dispatchSessionSSE(page, {
      type: "session.status",
      properties: { sessionID: SESSION_ID, status: { type: "idle" } },
    });
    await dispatchSessionSSE(page, {
      type: "session.status",
      properties: { sessionID: SESSION_ID, status: { type: "busy" } },
    });

    await expect(page.locator(".status-bar-status")).toHaveText("busy", { timeout: 3_000 });

    // Back to idle
    await dispatchSessionSSE(page, {
      type: "session.status",
      properties: { sessionID: SESSION_ID, status: { type: "idle" } },
    });

    await expect(page.locator(".status-bar-status")).toHaveText("ready", { timeout: 3_000 });
  });
});

// ── Tests: New session flow ───────────────────────────

test.describe("SSE: New session via + button", () => {
  test("clicking + creates new session and re-enables input", async ({ page }) => {
    const mocks = await setupAndNavigate(page);

    // 1. Initially the textarea should be enabled (active session exists)
    const textarea = page.locator(".prompt-textarea");
    await expect(textarea).toBeVisible();
    await expect(textarea).not.toBeDisabled();
    await expect(textarea).not.toHaveAttribute("placeholder", /Select a session/);

    // Model chip should be enabled
    const modelChip = page.locator(".prompt-chip").first();
    await expect(modelChip).not.toBeDisabled();

    // 2. When the user clicks "+", the backend clears active_session.
    //    Simulate this: after POST /api/session/new succeeds, refreshState()
    //    returns state with active_session = null.
    const stateWithNoActiveSession = {
      ...MOCK_APP_STATE,
      projects: [
        {
          ...MOCK_APP_STATE.projects[0],
          active_session: null,
        },
      ],
    };
    mocks.setAppState(stateWithNoActiveSession);

    // Click the "+" new session button
    const newBtn = page.locator(".sb-new-btn");
    await expect(newBtn).toBeVisible();
    await newBtn.click();

    // 3. The textarea and model chip should become disabled
    await expect(textarea).toBeDisabled({ timeout: 3_000 });
    await expect(textarea).toHaveAttribute("placeholder", /Select a session/);
    await expect(modelChip).toBeDisabled();

    // 4. Simulate what happens when the backend creates a session:
    //    The server emits state_changed SSE, and refreshState() now returns
    //    a state with the new session as active.
    const NEW_SESSION_ID = "ses_new_from_plus";
    const stateWithNewSession = {
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
    };
    mocks.setAppState(stateWithNewSession);

    // Mock the messages endpoint for the new session
    await page.route(`**/api/session/${NEW_SESSION_ID}/messages*`, (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ messages: [], total: 0 }),
      })
    );

    // Dispatch state_changed SSE event (backend sends this after session.created)
    await dispatchAppSSE(page, "state_changed", "");

    // 5. The textarea and model chip should be re-enabled
    await expect(textarea).not.toBeDisabled({ timeout: 5_000 });
    await expect(textarea).not.toHaveAttribute("placeholder", /Select a session/);
    await expect(modelChip).not.toBeDisabled({ timeout: 3_000 });

    // 6. The new session should appear in the sidebar
    await expect(page.locator(".sb-session-title", { hasText: "New Session" })).toBeVisible({ timeout: 3_000 });

    // 7. User should be able to type in the textarea
    await textarea.fill("Hello from new session");
    await expect(textarea).toHaveValue("Hello from new session");
  });
});
