import { test, expect, type Page } from "@playwright/test";

type RunnerCase = {
  name: string;
  modelQuery: string;
  providerText: string;
  permission: string;
};

const RUNNERS: RunnerCase[] = [
  { name: "opencode", modelQuery: "luna", providerText: "OpenAI", permission: "default" },
  { name: "codex", modelQuery: "luna", providerText: "OpenAI", permission: "never" },
  { name: "claude", modelQuery: "haiku", providerText: "Anthropic", permission: "plan" },
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
    const runnerButton = page.locator('button[title="Choose runner, effort, and permissions"]');
    await runnerButton.click();
    await page.getByRole("menuitemradio", { name: runner.name, exact: true }).click();
    await runnerButton.click();

    await page.locator('button[title="Change model"]').click();
    const modelPicker = page.locator('[role="dialog"][aria-label="Choose model"]');
    await modelPicker.locator(".model-picker-input").fill(runner.modelQuery);
    const matchingModels = modelPicker.locator(".model-picker-item").filter({ hasText: runner.providerText });
    await expect.poll(async () => matchingModels.count(), {
      timeout: 60_000,
    }).toBeGreaterThan(0);
    await matchingModels.first().click();
    await expect(modelPicker).not.toBeVisible();

    await runnerButton.click();
    const lowEffort = page.getByRole("radio", { name: "low", exact: true });
    if (await lowEffort.count()) await lowEffort.click();
    await page.getByLabel("Runner permissions").selectOption(runner.permission);
    await runnerButton.click();
  }

  async function sendAndWait(page: Page, runner: string, number: number, previousReplies: number) {
    const marker = `REAL_RUNNER_OK_${runner}_${number}`;
    await page.locator(".prompt-textarea").fill(`Reply with exactly ${marker}.`);
    await page.locator(".prompt-send-btn").click();
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
    // Claude's CLI reports idle just before its resumed-process handoff is
    // fully released. Give that handoff a short settling window before the
    // next prompt, otherwise the follow-up is stranded as queued.
    if (number === 1) await page.waitForTimeout(5_000);
    return previousReplies + 1;
  }

  test("gets two real replies from every runner using requested cheap models", async ({ page }) => {
    await login(page);
    for (const runner of SELECTED_RUNNERS) {
      await page.locator(".sb-new-btn").click();
      await expect(page).toHaveURL(/new=1/);
      await configure(page, runner);
      let replies = 0;
      replies = await sendAndWait(page, runner.name, 1, replies);
      replies = await sendAndWait(page, runner.name, 2, replies);
      expect(replies).toBe(2);
    }
  });
});
