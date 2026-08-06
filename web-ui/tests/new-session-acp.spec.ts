/**
 * Test: the first turn of a new session on an ACP runner.
 *
 * The ACP engine's send route is fire-and-forget, so the message POST returns before the
 * prompt has been folded into the transcript — the refetch that follows a send sees an
 * empty conversation, and everything the user sees for that turn arrives over SSE instead.
 * That ordering is what these tests pin, because it is where a real regression landed: the
 * prompt rendered twice and the transcript claimed "No response" with the reply on screen.
 *
 * The event sequence replayed here was captured from the live engine, not invented.
 */

import { test, expect, type Page } from "@playwright/test";
import { setupDynamicMocks } from "./newSessionMocks";
import {
  installEventSourceSpy,
  dispatchAppSSE,
  claudeBackendState,
  sessionRow,
  login,
  send,
  FIRST_CREATED_ID,
} from "./newSessionFixtures";

/** Timestamps as the engine really sent them. See `serverTimes` for why they matter. */
const CAPTURED = { user: 1_786_008_743_856, assistant: 1_786_008_754_685 };
const ASSISTANT_ID = "msg_011CdmGF1n7YbwgKR6GrarLr";

/** A 2x2 red PNG as the composer sends it: a data URL the timeline can put in an `<img>`. */
const RED_PNG =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91Jpz" +
  "AAAAEElEQVR4nGP4z8AARAwQCgAf7gP9i18U1AAAAABJRU5ErkJggg==";

/** A turn here costs ~25s of wall clock: the "No response" verdict is only reached 8s
 *  after the session goes idle, and the fixture waits out the stream churn first. */
test.describe.configure({ timeout: 120_000 });

type Times = { user: number; assistant: number };

/**
 * Where the server's clock sits relative to the browser's.
 *
 * The optimistic placeholder is stamped with the browser's `Date.now()` and the confirmed
 * record with the server's, so which of the two is larger is a real variable in this flow
 * — worth naming and varying rather than leaving as an incidental constant.
 */
function serverTimes(skew: "behind" | "ahead"): Times {
  if (skew === "behind") return CAPTURED;
  const base = Date.now() + 30_000;
  return { user: base, assistant: base + 10_000 };
}

/**
 * Push one engine frame into the session-events EventSource, the way the app really
 * receives them: event name `opencode`, JSON payload.
 *
 * Closed sources are skipped. StrictMode mounts the SSE hook twice, so a torn-down
 * EventSource is still in the spy's list — delivering to it as well would apply every
 * `message.part.delta` twice and turn "pong" into "pongong".
 */
async function emit(page: Page, type: string, properties: Record<string, unknown>) {
  const delivered = await page.evaluate((frame) => {
    const ev = new MessageEvent("opencode", { data: JSON.stringify(frame) });
    let count = 0;
    for (const es of ((window as any).__eventSources || []) as EventSource[]) {
      if (!es.url || !es.url.includes("/api/session/events")) continue;
      if (es.readyState === 2 /* CLOSED */) continue;
      es.dispatchEvent(ev);
      count++;
    }
    return count;
  }, { type, properties });
  expect(delivered, `frame ${type} must reach exactly one live stream`).toBe(1);
  // One frame per tick, so the hook's 16ms message flush lands between them exactly as it
  // does against a live stream.
  await page.waitForTimeout(30);
}

/**
 * Replay the engine's real frames for one turn.
 *
 * Two details matter and are easy to lose when hand-writing a fixture. The user message's
 * envelope arrives one event *before* its text, so any logic keyed on the message's content
 * sees nothing on the first frame. And the assistant's first chunk is a whole part while the
 * rest are `message.part.delta` appends, so the reply only reads correctly if both are
 * applied to the same part.
 */
async function replayTurn(
  page: Page,
  sid: string,
  prompt: string,
  t: Times,
  opts: { reply?: boolean; attachment?: boolean } = {},
) {
  const userId = `msg_user_${sid}_1`;
  await emit(page, "message.updated", {
    info: { id: userId, role: "user", sessionID: sid, time: { created: t.user } },
  });
  await emit(page, "message.part.updated", {
    sessionID: sid,
    time: t.user,
    part: { type: "text", id: `${userId}:0`, messageID: userId, sessionID: sid, text: prompt },
  });
  if (opts.attachment) {
    // The engine emits an upload as a second part on the same user message.
    await emit(page, "message.part.updated", {
      sessionID: sid,
      time: t.user,
      part: {
        type: "file", id: `${userId}:1`, messageID: userId, sessionID: sid,
        mime: "image/png", filename: "red.png", url: RED_PNG,
      },
    });
  }
  await emit(page, "session.status", { sessionID: sid, status: { type: "busy" } });

  if (opts.reply !== false) {
    await emit(page, "message.updated", {
      info: {
        id: ASSISTANT_ID, role: "assistant", sessionID: sid,
        model: "", cost: 0, tokens: {}, time: { created: t.assistant },
      },
    });
    await emit(page, "message.part.updated", {
      sessionID: sid,
      time: t.assistant,
      part: {
        type: "text", id: `${ASSISTANT_ID}:0`, messageID: ASSISTANT_ID,
        sessionID: sid, text: "p",
      },
    });
    await emit(page, "message.part.delta", {
      sessionID: sid, messageID: ASSISTANT_ID, partID: `${ASSISTANT_ID}:0`,
      field: "text", delta: "ong",
    });
    await emit(page, "message.updated", {
      info: {
        id: ASSISTANT_ID, role: "assistant", sessionID: sid, model: "", cost: 0.0217,
        tokens: { input: 2, output: 4, reasoning: 0, cache: { read: 42033, write: 0 } },
        time: { created: t.assistant, completed: t.assistant + 541 },
      },
    });
  }

  await emit(page, "session.status", { sessionID: sid, status: { type: "idle" } });
}

/** Visible text of every rendered user turn. */
async function userBubbleTexts(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    Array.from(document.querySelectorAll(".message-turn-user"), (n) =>
      (n.textContent || "").trim()
    )
  );
}

/** How many user turns render the prompt — counted, not merely checked for presence. */
async function promptBubbleCount(page: Page, prompt: string): Promise<number> {
  return (await userBubbleTexts(page)).filter((text) => text.includes(prompt)).length;
}

/**
 * Answer each SSE endpoint once, then leave every reconnect pending.
 *
 * A live stream stays open for the whole turn. A mock that closes after its body turns into
 * a reconnect loop instead, and every reconnect runs `recoverAfterReconnect()` — a fresh
 * `refreshMessages()` that reconciles the transcript as a side effect. Production has no
 * such repair mid-turn, and letting it happen here would hide the defect under test.
 */
async function pinSSE(page: Page) {
  for (const pattern of ["**/api/events*", "**/api/session/events*"]) {
    let served = 0;
    await page.route(pattern, async (route) => {
      // Two: StrictMode's double mount opens each stream twice.
      if (served++ >= 2) await new Promise((resolve) => setTimeout(resolve, 60_000));
      return route.fulfill({ status: 200, contentType: "text/event-stream", body: ": open\n\n" });
    });
  }
}

/** Wait until no further transcript fetch has been issued for `quietMs`. */
async function quiesce(page: Page, fetches: () => number, quietMs: number) {
  let last = -1;
  while (last !== fetches()) {
    last = fetches();
    await page.waitForTimeout(quietMs);
  }
}

async function startAcpSession(page: Page) {
  await installEventSourceSpy(page);
  const mocks = await setupDynamicMocks(page);
  // No active session, so the first send has to create one — the path under test.
  mocks.setAppState(claudeBackendState([], null));

  const newBodies: any[] = [];
  /**
   * The backend registers the session as active before it answers `/session/new`, so the
   * next state snapshot names it. A fixture that kept reporting "no active session" would
   * drop the client back onto the new-session screen and orphan the turn — an artefact of
   * the mock, not the defect.
   */
  await page.route("**/api/session/new", (route) => {
    newBodies.push(route.request().postDataJSON());
    mocks.setAppState(claudeBackendState(
      [sessionRow(FIRST_CREATED_ID, "New Session", "claude")],
      FIRST_CREATED_ID,
    ));
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ session_id: FIRST_CREATED_ID }),
    });
  });

  /**
   * What every `GET /session/{id}/messages` returns. It stays EMPTY: the engine's send
   * route is fire-and-forget, so the prompt is not in the transcript yet when the send's
   * refetch reads it — and nothing fetches again while the turn streams.
   */
  const transcript = { fetches: 0 };
  await page.route("**/api/session/*/messages*", (route) => {
    transcript.fetches += 1;
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ messages: [], has_more: false, total: 0 }),
    });
  });

  await pinSSE(page);
  await login(page, "/");
  // Let the streams reach their pinned state before anything is measured.
  await quiesce(page, () => transcript.fetches, 2500);
  return { mocks, transcript, newBodies };
}

/** Send the first prompt, then replay the engine's frames for that turn. */
async function firstTurn(
  page: Page,
  opts: { reply?: boolean; skew?: "behind" | "ahead"; attachment?: boolean } = {},
) {
  const ctx = await startAcpSession(page);
  await send(page, "Hello");
  await expect(page).toHaveURL(new RegExp(`session=${FIRST_CREATED_ID}`));
  // The send awaits a transcript refetch; let it land, so the turn starts from the state
  // production reaches — prompt sent, transcript still empty.
  await quiesce(page, () => ctx.transcript.fetches, 1500);
  expect(ctx.transcript.fetches, "the send must have refetched the transcript").toBeGreaterThan(0);
  await dispatchAppSSE(page, "state_changed", "");

  const before = ctx.transcript.fetches;
  await replayTurn(page, FIRST_CREATED_ID, "Hello", serverTimes(opts.skew ?? "behind"), opts);
  // Nothing may refetch the transcript mid-turn: a stray fetch reconciles the message map
  // as a side effect, which would make the assertions below pass for the wrong reason.
  expect(ctx.transcript.fetches, "no transcript refetch during the turn").toBe(before);
  return ctx;
}

test.describe("New session on an ACP runner", () => {
  /**
   * The duplicate-prompt regression. The transcript refetch after a send returns nothing
   * (the engine has not folded the prompt in yet), so the local placeholder is still the
   * only copy on screen when the confirmed record arrives over SSE. It has to be retired by
   * matching the prompt's *text* — but the confirmed record's text arrives one frame later
   * than its envelope, and the timestamp fallback keeps both copies whenever the browser's
   * clock leads the server's.
   */
  test("renders the prompt exactly once", async ({ page }) => {
    await firstTurn(page);

    await expect.poll(() => promptBubbleCount(page, "Hello")).toBe(1);
  });

  /** A reply split across `message.part.updated` + `message.part.delta` must compose. */
  test("renders the streamed reply", async ({ page }) => {
    await firstTurn(page);

    await expect(page.locator(".message-turn-assistant")).toContainText("pong");
  });

  /**
   * "No response" is keyed on the *last* message being the user's. A placeholder that was
   * never retired sorts newest — the browser stamped it — so the banner appears even with
   * the reply rendered directly above it. Two symptoms, one cause.
   */
  test("does not claim No response once the reply has arrived", async ({ page }) => {
    await firstTurn(page);

    await expect(page.locator(".message-turn-assistant")).toContainText("pong");
    // The verdict is only reached 8s after the session goes idle, so an immediate absence
    // would prove nothing — wait past the grace window first.
    await page.waitForTimeout(10_000);
    await expect(page.locator(".pending-reply")).toHaveCount(0);
    await expect(page.getByText("No response", { exact: false })).toHaveCount(0);
  });

  /**
   * Proves the assertion above can fail: with the assistant frames withheld, the turn really
   * is unanswered and the banner must appear. Without this, "No response is absent" would
   * pass for any reason at all, including the timeline never rendering.
   */
  test("does claim No response when the runner truly sends nothing", async ({ page }) => {
    await firstTurn(page, { reply: false });

    await expect(page.locator(".pending-reply")).toContainText("No response", { timeout: 20_000 });
  });

  /**
   * The same sequence with the server's clock *ahead* of the browser's. Isolating that one
   * variable shows what the placeholder's retirement actually depends on: this turn passes
   * while the identical turn with a trailing server clock does not.
   */
  test("renders the prompt exactly once when the server clock leads the browser's", async ({ page }) => {
    await firstTurn(page, { skew: "ahead" });

    await expect(page.locator(".message-turn-assistant")).toContainText("pong");
    await expect.poll(() => promptBubbleCount(page, "Hello")).toBe(1);
  });

  /**
   * One prompt, one POST. The first send creates the session and then sends into it; a
   * second POST here would mean the turn was submitted twice, which is its own way of
   * showing the prompt twice.
   */
  test("posts the prompt exactly once", async ({ page }) => {
    const posted: string[] = [];
    page.on("request", (req) => {
      const path = new URL(req.url()).pathname;
      if (req.method() === "POST" && path.endsWith("/message")) posted.push(path);
    });

    const { mocks, newBodies } = await firstTurn(page);

    expect(posted).toEqual([`/api/session/${FIRST_CREATED_ID}/message`]);
    expect(mocks.sentMessages).toHaveLength(1);
    expect(mocks.sentMessages[0].sessionId).toBe(FIRST_CREATED_ID);
    expect(mocks.sentMessages[0].body.parts[0].text).toBe("Hello");
    // One session, created for the ACP runner.
    expect(newBodies).toHaveLength(1);
    expect(newBodies[0].runner).toBe("claude");
  });

  /**
   * File upload, the half the user sees. The engine sends an attachment as a second `file`
   * part on the user's own message; the timeline has always known how to render one, so the
   * requirement is that the part reaches it at all — for a long time ACP dropped uploads
   * before they ever became a part.
   */
  test("previews an attached image in the user's own message", async ({ page }) => {
    await firstTurn(page, { attachment: true });

    const thumb = page.locator(".message-turn-user .message-image-thumb img");
    await expect(thumb).toHaveCount(1);
    await expect(thumb).toHaveAttribute("src", RED_PNG);
    await expect(page.locator(".message-turn-user .message-image-name")).toHaveText("red.png");
  });

  /** The attachment must not cost the prompt its text, nor duplicate the bubble. */
  test("an attached image leaves the prompt itself intact", async ({ page }) => {
    await firstTurn(page, { attachment: true });

    expect(await promptBubbleCount(page, "Hello")).toBe(1);
    await expect(page.locator(".message-turn-assistant")).toContainText("pong");
  });
});
