/**
 * Page-level fixtures for the new-session specs: the SSE spy the tests push
 * state changes through, the app-state shapes they assert against, and the
 * login/send steps every case repeats.
 */

import { Page } from "@playwright/test";
import { MOCK_APP_STATE } from "./helpers";

export const HANDOFF_SESSION_ID = "ses_handoff_from_runner_switch";
export const FIRST_CREATED_ID = "ses_new_from_first_send_1";

/**
 * Install EventSource spy to dispatch SSE events from tests.
 */
export async function installEventSourceSpy(page: Page) {
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

export async function dispatchAppSSE(page: Page, eventType: string, data: string) {
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

/** App state for a claude-code install: `backend` cannot name a runner there. */
export function claudeBackendState(sessions: any[], activeSession: string | null) {
  return {
    ...MOCK_APP_STATE,
    backend: "claude-code",
    default_runner: "claude",
    runners: ["claude", "claude-code", "codex", "opencode"],
    projects: [{ ...MOCK_APP_STATE.projects[0], active_session: activeSession, sessions }],
  };
}

export function sessionRow(id: string, title: string, runner: string) {
  return {
    id, title, parentID: "", runner,
    directory: "/home/user/my-project",
    time: { created: 1700005000, updated: 1700005000 },
  };
}

export async function login(page: Page, url: string) {
  await page.goto(url);
  await page.evaluate(() => sessionStorage.setItem("opman_token", "mock-jwt-token"));
  await page.reload({ waitUntil: "networkidle" });
  await page.waitForSelector(".chat-layout", { timeout: 15_000 });
}

export async function send(page: Page, text: string) {
  await page.locator(".prompt-textarea").fill(text);
  const posted = page.waitForRequest((request) => (
    request.method() === "POST" && new URL(request.url()).pathname.endsWith("/message")
  ));
  await page.locator(".prompt-send-btn").click();
  await posted;
}
