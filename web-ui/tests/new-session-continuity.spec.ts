/**
 * Test: a session, once created, keeps every following turn.
 *
 * opman can hand a session off to another runner, which means minting a new
 * runner-native session and replaying a summary into it. That is right when the
 * user picks a different runner and wrong every other time — an ordinary
 * follow-up prompt must continue the conversation it belongs to. These tests pin
 * both halves of that rule at the request level, because the symptom (a fresh
 * "Handoff session" appearing on the second prompt) is only visible in what the
 * client sends, not in what it renders.
 */

import { test, expect } from "@playwright/test";
import { SESSION_ID, MOCK_APP_STATE } from "./helpers";
import { setupDynamicMocks } from "./newSessionMocks";
import {
  installEventSourceSpy, dispatchAppSSE, claudeBackendState, sessionRow, login, send,
  HANDOFF_SESSION_ID, FIRST_CREATED_ID,
} from "./newSessionFixtures";
import { chooseRunner, closeEnginePalette } from "./enginePicker";

test.describe("Session continuity across runners", () => {
  /**
   * The regression this file exists for: a second prompt must continue the
   * session the first one created.
   *
   * The trap is the runner label. `POST /api/session/new` creates the
   * runner-native session before recording its runner, so a snapshot taken in
   * that window reports the *default* runner for a session that belongs to
   * another one — and the browser may hold that snapshot for the whole next
   * turn. Anything that re-derives the runner per send therefore asks the wrong
   * runner, which the backend reads as a switch and answers by forking the
   * conversation into a handoff session. So: a send on an existing session must
   * carry no runner at all.
   */
  test("a follow-up prompt continues the created session instead of forking it", async ({ page }) => {
    await installEventSourceSpy(page);
    const mocks = await setupDynamicMocks(page);
    mocks.setAppState(claudeBackendState(MOCK_APP_STATE.projects[0].sessions, null));
    mocks.handOffOn("claude-code");
    await login(page, "/?new=1");

    await send(page, "first prompt");
    await expect(page).toHaveURL(new RegExp(`session=${FIRST_CREATED_ID}`));
    // The session is created for the server's declared default runner — never a
    // runner guessed from `backend`, which reports "claude-code" for both
    // claude engines.
    expect(mocks.newSessionBodies).toHaveLength(1);
    expect(mocks.newSessionBodies[0].runner).toBe("claude");

    // The label race: state now reports the new session as owned by the
    // *default* runner label of a claude-code install, not the one it was
    // created with.
    mocks.setAppState(claudeBackendState(
      [...MOCK_APP_STATE.projects[0].sessions, sessionRow(FIRST_CREATED_ID, "New Session", "claude-code")],
      FIRST_CREATED_ID,
    ));
    await dispatchAppSSE(page, "state_changed", "");

    await send(page, "second prompt");

    // Same session, no second creation, and nothing that could trigger a handoff.
    expect(mocks.newSessionBodies).toHaveLength(1);
    expect(mocks.sentMessages).toHaveLength(2);
    expect(mocks.sentMessages[1].sessionId).toBe(FIRST_CREATED_ID);
    expect(mocks.sentMessages[0].sessionId).toBe(FIRST_CREATED_ID);
    expect(mocks.sentMessages[1].body).not.toHaveProperty("runner");
    await expect(page).toHaveURL(new RegExp(`session=${FIRST_CREATED_ID}`));
    await expect(page).not.toHaveURL(new RegExp(HANDOFF_SESSION_ID));
  });

  /** The one case that should hand off: the user picks a different runner. */
  test("switching the runner hands off once, then keeps sending to the new session", async ({ page }) => {
    await installEventSourceSpy(page);
    const mocks = await setupDynamicMocks(page);
    const existing = sessionRow(SESSION_ID, "Test Session", "claude");
    mocks.setAppState(claudeBackendState([existing], SESSION_ID));
    mocks.handOffOn("claude-code");
    await login(page, `/?session=${SESSION_ID}`);

    await chooseRunner(page, "claude-code");
    await closeEnginePalette(page);

    await send(page, "switch me");
    expect(mocks.sentMessages[0].body.runner).toBe("claude-code");
    await expect(page).toHaveURL(new RegExp(`session=${HANDOFF_SESSION_ID}`));

    mocks.setAppState(claudeBackendState(
      [existing, sessionRow(HANDOFF_SESSION_ID, "Handoff session", "claude-code")],
      HANDOFF_SESSION_ID,
    ));
    await dispatchAppSSE(page, "state_changed", "");

    // The pick must not survive the handoff, or every later turn forks again.
    await send(page, "follow-up");
    expect(mocks.sentMessages).toHaveLength(2);
    expect(mocks.sentMessages[1].sessionId).toBe(HANDOFF_SESSION_ID);
    expect(mocks.sentMessages[1].body).not.toHaveProperty("runner");
  });

});
