import { test, expect, type Page } from "@playwright/test";
import { chooseRunner, setEngineSettings, closeEnginePalette } from "./enginePicker";

type RunnerCase = {
  name: string;
  modelQuery: string;
  providerText: string;
  permission: string;
};

const RUNNERS: RunnerCase[] = [
  { name: "opencode", modelQuery: "luna", providerText: "OpenAI", permission: "default" },
  { name: "codex", modelQuery: "luna", providerText: "OpenAI", permission: "never" },
  // Not "plan": claude's plan mode ends the turn without writing anything to
  // the transcript, so a test asserting a reply can never pass under it.
  { name: "claude", modelQuery: "haiku", providerText: "Anthropic", permission: "default" },
  { name: "claude-code", modelQuery: "haiku", providerText: "Anthropic", permission: "acceptEdits" },
];
const SELECTED_RUNNERS = process.env.OPMAN_REAL_RUNNER
  ? RUNNERS.filter((runner) => runner.name === process.env.OPMAN_REAL_RUNNER)
  : RUNNERS;

test.describe("Real new-session runner responses", () => {
  test.skip(!process.env.OPMAN_REAL_RUNNERS, "Set OPMAN_REAL_RUNNERS=1 to spend real model tokens");
  test.setTimeout(15 * 60 * 1000);

  async function login(page: Page) {
    await page.goto(process.env.OPMAN_REAL_URL || "http://127.0.0.1:9090", { waitUntil: "networkidle" });
    if (!await page.locator(".login-container").isVisible().catch(() => false)) return;
    await page.locator('input[type="text"]').fill(process.env.OPMAN_REAL_USER || "admin");
    await page.locator('input[type="password"]').fill(process.env.OPMAN_REAL_PASSWORD || "");
    await page.locator('button[type="submit"]').click();
    await page.waitForSelector(".chat-layout", { timeout: 180_000 });
  }

  async function configure(page: Page, runner: RunnerCase) {
    await chooseRunner(page, runner.name);

    // Same palette, now listing that runner's models: search, then take the
    // first match from the expected provider.
    const palette = page.locator('[role="dialog"][aria-label="Choose runner, model, and agent"]');
    await palette.locator(".engine-palette-input").fill(runner.modelQuery);
    const matches = palette.getByRole("option").filter({ hasText: runner.providerText });
    await expect.poll(async () => matches.count(), { timeout: 60_000 }).toBeGreaterThan(0);
    await matches.first().click();
    await expect(palette).not.toBeVisible();

    await setEngineSettings(page, { effort: "low", permission: runner.permission });
    await closeEnginePalette(page);
  }

  async function sendAndWait(page: Page, runner: string, number: number, previousReplies: number) {
    const marker = `REAL_RUNNER_OK_${runner}_${number}`;
    await page.locator(".prompt-textarea").fill(`Reply with exactly ${marker}.`);
    await page.locator(".prompt-send-btn").click();

    // The submitted prompt must show up straight away and stay put. On a
    // session's first send there is no transcript to fall back on, so this is
    // the guard against the composer clearing into a blank new-session screen.
    const userTurn = page.locator(".message-turn-user").filter({ hasText: marker });
    await expect(userTurn).toBeVisible({ timeout: 15_000 });
    await expect(page.locator(".new-session-welcome")).toHaveCount(0);

    // Until the reply lands, something has to say the turn is in flight —
    // either the pending placeholder or the assistant turn itself.
    await expect(page.locator(".pending-reply, .message-turn-assistant").first())
      .toBeVisible({ timeout: 15_000 });

    await expect.poll(async () => {
      const replies = await page.locator(".message-turn-assistant").allTextContents();
      const failure = replies.find((text) => /insufficient balance|apierror|error/i.test(text));
      if (failure) throw new Error(`${runner} returned an error: ${failure.slice(0, 240)}`);
      return replies.filter((text) => text.includes(marker)).length;
    }, { timeout: 180_000, message: `${runner} did not return reply ${number}` }).toBeGreaterThan(previousReplies);
    await expect.poll(async () => page.evaluate(async () => {
      const response = await fetch("/api/state");
      const state = await response.json();
      const project = state.projects?.[state.active_project];
      return project?.busy_sessions?.includes(project.active_session) ?? false;
    }), { timeout: 180_000, message: `${runner} remained busy after reply ${number}` }).toBe(false);

    // Exactly one copy of the prompt: the placeholder must be retired by the
    // transcript record, not left alongside it.
    await expect(userTurn).toHaveCount(1);
    // The in-flight placeholder must clear once the session goes idle.
    await expect(page.locator(".pending-reply")).toHaveCount(0, { timeout: 30_000 });
    // Claude's CLI reports idle just before its resumed-process handoff is
    // fully released. Give that handoff a short settling window before the
    // next prompt, otherwise the follow-up is stranded as queued.
    if (number === 1) await page.waitForTimeout(5_000);
    return previousReplies + 1;
  }

  /** The session the URL currently points at — empty on the new-session screen. */
  function activeSession(page: Page): string {
    return new URL(page.url()).searchParams.get("session") || "";
  }

  test("gets two real replies from every runner using requested cheap models", async ({ page }) => {
    await login(page);
    for (const runner of SELECTED_RUNNERS) {
      await page.locator(".sb-new-btn").click();
      await expect(page).toHaveURL(/new=1/);
      await configure(page, runner);
      let replies = 0;
      replies = await sendAndWait(page, runner.name, 1, replies);
      // The first send creates the session; every later send must land in it.
      // A second session appearing here means the follow-up was read as a runner
      // switch and forked into a handoff session, losing the conversation.
      const sessionId = activeSession(page);
      expect(sessionId).not.toBe("");
      replies = await sendAndWait(page, runner.name, 2, replies);
      expect(activeSession(page)).toBe(sessionId);
      expect(replies).toBe(2);
      // Both turns share one transcript: reply 1 is still on screen after send 2.
      // A handoff would have swapped in a fresh, near-empty transcript instead.
      await expect(page.locator(".message-turn-assistant")
        .filter({ hasText: `REAL_RUNNER_OK_${runner.name}_1` })).toHaveCount(1);
    }
  });
});
