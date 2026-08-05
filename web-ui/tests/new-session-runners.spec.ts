import { test, expect, type Page } from "@playwright/test";
import { MOCK_THEME, setupMockAPI } from "./helpers";

type RunnerCase = {
  runner: string;
  provider: string;
  model: string;
  permission: string;
};

const RUNNERS: RunnerCase[] = [
  { runner: "opencode", provider: "fixture-opencode", model: "qwen3-coder-free", permission: "default" },
  { runner: "claude-code", provider: "fixture-claude-code", model: "claude-3-haiku", permission: "acceptEdits" },
  { runner: "claude", provider: "fixture-claude", model: "claude-3-haiku", permission: "plan" },
  { runner: "codex", provider: "fixture-codex", model: "gpt-4o-mini", permission: "never" },
];

function providerFixture(testCase: RunnerCase) {
  return {
    all: [{
      id: testCase.provider,
      name: `${testCase.runner} cheapest`,
      models: {
        [testCase.model]: {
          id: testCase.model,
          name: testCase.model,
          reasoningEfforts: ["low", "medium", "high"],
          limit: { context: 128000, output: 16000 },
        },
      },
    }],
    connected: [testCase.provider],
    default: { [testCase.provider]: testCase.model },
  };
}

async function installRunnerFixtures(page: Page) {
  await setupMockAPI(page);
  let createdSessions = 0;

  await page.route(/\/api\/providers(?:\?.*)?$/, (route) => {
    const runner = new URL(route.request().url()).searchParams.get("runner") || "opencode";
    const testCase = RUNNERS.find((item) => item.runner === runner) || RUNNERS[0];
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(providerFixture(testCase)),
    });
  });

  await page.route("**/api/session/new", (route) => {
    createdSessions += 1;
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ session_id: `fixture-new-${createdSessions}` }),
    });
  });

  await page.route("**/api/theme", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MOCK_THEME) }),
  );
}

async function navigateRunnerTest(page: Page) {
  await page.goto("/");
  await page.evaluate(() => sessionStorage.setItem("opman_token", "mock-jwt-token"));
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });
}

async function chooseRunnerSettings(page: Page, testCase: RunnerCase) {
  const runnerButton = page.locator('button[title="Choose runner, effort, and permissions"]');
  await runnerButton.click();
  await page.getByRole("menuitemradio", { name: testCase.runner, exact: true }).click();
  await runnerButton.click();
}

async function chooseCheapestModel(page: Page, testCase: RunnerCase) {
  await page.locator('button[title="Change model"]').click();
  const modal = page.locator('[role="dialog"][aria-label="Choose model"]');
  await expect(modal).toBeVisible();
  await expect(modal.locator(".model-picker-item")).toHaveCount(1);
  await expect(modal.locator(".model-picker-item")).toContainText(testCase.model);
  await modal.locator(".model-picker-item").click();
  await expect(modal).not.toBeVisible();
}

async function sendAndAssert(page: Page, testCase: RunnerCase, text: string) {
  const requestPromise = page.waitForRequest((request) => {
    return request.method() === "POST" && new URL(request.url()).pathname.endsWith("/message");
  });
  const textarea = page.locator(".prompt-textarea");
  await textarea.fill(text);
  await textarea.press("Enter");
  const request = await requestPromise;
  const body = request.postDataJSON();

  expect(body.parts).toEqual([{ type: "text", text }]);
  expect(body.runner).toBe(testCase.runner);
  expect(body.model).toEqual({ providerID: testCase.provider, modelID: testCase.model });
  expect(body.effort).toBe("low");
  expect(body.permission).toBe(testCase.permission);
  await expect(textarea).toBeEnabled({ timeout: 10_000 });
}

test.describe("New session runner configuration", () => {
  test("selects cheapest model, effort, and permission, then sends twice per runner", async ({ page }) => {
    await installRunnerFixtures(page);
    await navigateRunnerTest(page);
    await expect(page.locator(".chat-layout")).toBeVisible();

    for (const testCase of RUNNERS) {
      await page.locator(".sb-new-btn").click();
      await expect(page).toHaveURL(/new=1/);
      await chooseRunnerSettings(page, testCase);
      await chooseCheapestModel(page, testCase);
      const runnerButton = page.locator('button[title="Choose runner, effort, and permissions"]');
      await runnerButton.click();
      await page.getByRole("radio", { name: "low", exact: true }).click();
      await page.getByLabel("Runner permissions").selectOption(testCase.permission);
      await runnerButton.click();
      await sendAndAssert(page, testCase, `${testCase.runner} first message`);
      await sendAndAssert(page, testCase, `${testCase.runner} second message`);
    }
  });
});
