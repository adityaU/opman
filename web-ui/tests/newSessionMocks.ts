/**
 * Mock API surface shared by the new-session specs.
 *
 * Every route the chat layout touches is stubbed here, so a spec only has to
 * describe the state transitions it cares about. The returned controllers record
 * what the app actually sent — which session, with which runner — because that
 * is what the session-continuity assertions are about.
 */

import { Page } from "@playwright/test";
import {
  MOCK_APP_STATE,
  MOCK_STATS,
  MOCK_COMMANDS,
  MOCK_PROVIDERS,
  MOCK_THEME,
} from "./helpers";
import { HANDOFF_SESSION_ID } from "./newSessionFixtures";

export async function setupDynamicMocks(page: Page, options: { messageDelayMs?: number } = {}) {
  let currentAppState: any = MOCK_APP_STATE;
  let createdSessionCount = 0;
  /** Every `/api/session/new` body, so tests can assert a session is created once. */
  const newSessionBodies: any[] = [];
  /** Every message POST as `{ sessionId, body }`, in order. */
  const sentMessages: { sessionId: string; body: any }[] = [];
  /** When set, a message naming a runner answers as a handoff to a new session. */
  let handoffRunner: string | null = null;

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
  await page.route("**/api/session/*/message", async (route) => {
    if (route.request().method() === "POST") {
      const body = route.request().postDataJSON();
      const sessionId = new URL(route.request().url()).pathname.split("/").at(-2) || "";
      sentMessages.push({ sessionId, body });
      if (options.messageDelayMs) await new Promise((resolve) => setTimeout(resolve, options.messageDelayMs));
      // Mirror the backend: a runner that differs from the session's own forks
      // the conversation into a handoff session.
      if (handoffRunner && body?.runner === handoffRunner) {
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({
            ok: true, switched: true, runner: handoffRunner,
            session_id: HANDOFF_SESSION_ID, response: { ok: true },
          }),
        });
      }
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

  await page.route("**/api/session/new", (route) => {
    newSessionBodies.push(route.request().postDataJSON());
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ session_id: `ses_new_from_first_send_${++createdSessionCount}` }),
    });
  });

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
    /** Make the message endpoint answer a send naming `runner` with a handoff. */
    handOffOn(runner: string | null) {
      handoffRunner = runner;
    },
    newSessionBodies,
    sentMessages,
  };
}
