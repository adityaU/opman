/**
 * Shared mock data and route-setup helpers used by all Playwright tests.
 *
 * Call `setupMockAPI(page)` inside each test (or in `beforeEach`) to install
 * route handlers that return realistic API responses. The helpers also set a
 * fake JWT so the app thinks the user is authenticated.
 */

import { Page } from "@playwright/test";

// ── Mock data ─────────────────────────────────────────

export const SESSION_ID = "ses_test_session_001";
export const SESSION_ID_2 = "ses_test_session_002";

export const MOCK_APP_STATE = {
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
  ],
  active_project: 0,
  panels: {
    sidebar: true,
    terminal_pane: false,
    neovim_pane: false,
    integrated_terminal: false,
    git_panel: false,
  },
  focused: "ChatInput",
};

export const MOCK_MESSAGES = [
  {
    info: {
      role: "user",
      messageID: "msg_001",
      time: 1700000100,
    },
    parts: [{ type: "text", text: "Hello, how are you?" }],
    metadata: {},
  },
  {
    info: {
      role: "assistant",
      messageID: "msg_002",
      time: 1700000200,
      model: { modelID: "claude-sonnet-4-20250514", providerID: "anthropic" },
    },
    parts: [
      {
        type: "text",
        text: "I'm doing well! Here is some code:\n\n```typescript\nconst x = 42;\nconsole.log(x);\n```\n\nLet me know if you need anything else.",
      },
    ],
    metadata: {
      cost: 0.0032,
      tokens: { input: 15, output: 45, reasoning: 0, cache_read: 0, cache_write: 0 },
    },
  },
  {
    info: {
      role: "assistant",
      messageID: "msg_003",
      time: 1700000300,
      model: { modelID: "claude-sonnet-4-20250514", providerID: "anthropic" },
    },
    parts: [
      {
        type: "tool-call",
        toolCallId: "tc_001",
        toolName: "read_file",
        args: { path: "/src/main.ts" },
      },
      {
        type: "tool-result",
        toolCallId: "tc_001",
        result: 'const app = express();\napp.listen(3000);',
      },
      {
        type: "text",
        text: "I read the file contents above.",
      },
    ],
    metadata: {
      cost: 0.005,
      tokens: { input: 100, output: 200, reasoning: 50, cache_read: 10, cache_write: 5 },
    },
  },
];

export const MOCK_STATS = {
  cost: 0.0082,
  input_tokens: 115,
  output_tokens: 245,
  reasoning_tokens: 50,
  cache_read: 10,
  cache_write: 5,
};

export const MOCK_COMMANDS = [
  { name: "new", description: "Start a new session" },
  { name: "model", description: "Switch model", args: "<model-name>" },
  { name: "compact", description: "Compact conversation history" },
  { name: "undo", description: "Undo last action" },
  { name: "redo", description: "Redo last action" },
  { name: "share", description: "Share session" },
];

export const MOCK_PROVIDERS = [
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
];

export const MOCK_THEME = {
  primary: "#7c3aed",
  secondary: "#10b981",
  accent: "#f59e0b",
  background: "#0f172a",
  background_panel: "#1e293b",
  background_element: "#334155",
  text: "#f1f5f9",
  text_muted: "#94a3b8",
  border: "#475569",
  border_active: "#7c3aed",
  border_subtle: "#334155",
  error: "#ef4444",
  warning: "#f59e0b",
  success: "#10b981",
  info: "#3b82f6",
};

// ── Route setup ───────────────────────────────────────

/**
 * Install mock API routes on `page` and inject a fake JWT into sessionStorage
 * so the app skips the login screen.
 */
export async function setupMockAPI(page: Page) {
  // Catch-all: prevent any unmocked /api/* request from hitting the real
  // backend (Vite proxies /api to 127.0.0.1:9090 which is not running in
  // tests). Playwright checks routes in REVERSE registration order, so
  // this is registered first (lowest priority) and specific routes below
  // override it.
  //
  // We use a function predicate instead of a glob like "**/api/**" because
  // the glob also matches Vite module-source requests for files under
  // src/api/ (e.g. /src/api/client.ts) and would break module loading.
  await page.route(
    (url) => url.pathname.startsWith("/api/"),
    (route) => route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({}) }),
  );

  // Mock all API routes before navigating
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

  await page.route("**/api/state", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MOCK_APP_STATE) })
  );

  await page.route(`**/api/session/${SESSION_ID}/messages*`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ messages: MOCK_MESSAGES, total: MOCK_MESSAGES.length }),
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

  // SSE endpoints — return empty streams that stay open briefly
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

  // POST endpoints — return success
  await page.route("**/api/session/*/message", (route) => {
    if (route.request().method() === "POST") {
      return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ok: true }) });
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
      body: JSON.stringify([
        { id: "default", name: "Default Agent", system_prompt: "" },
      ]),
    })
  );

  // ── OpenSpec assistant endpoints (memory, autonomy, routines, etc.) ──
  await page.route("**/api/memory/active*", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ memory: [] }) })
  );

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

  // ── Intelligence / assistant-center endpoints ──
  await page.route("**/api/recommendations", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ recommendations: [] }) })
  );

  await page.route("**/api/inbox", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ items: [], signals: [] }) })
  );

  await page.route("**/api/signals", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ signals: [] }) })
  );

  await page.route("**/api/resume-briefing", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({}) })
  );

  await page.route("**/api/daily-summary", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ summary: "" }) })
  );

  await page.route("**/api/assistant-center/stats", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({}) })
  );

  await page.route("**/api/workspace-templates", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ templates: [] }) })
  );
}

/**
 * Navigate to the app and inject a fake auth token so we skip login.
 */
export async function navigateAuthenticated(page: Page) {
  await setupMockAPI(page);

  // Navigate to the app
  await page.goto("/");

  // Inject the auth token into sessionStorage
  await page.evaluate(() => {
    sessionStorage.setItem("opman_token", "mock-jwt-token");
  });

  // Reload so the app picks up the token
  await page.reload();

  // Wait for the chat layout to be visible
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });
}
