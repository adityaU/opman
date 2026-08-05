import { test, expect, type Page } from "@playwright/test";
import { MOCK_THEME, setupMockAPI } from "./helpers";
import { chooseRunner, chooseModel, setEngineSettings, closeEnginePalette } from "./enginePicker";

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
  /** Ids handed out by `/api/session/new`, oldest first. */
  const createdSessionIds: string[] = [];

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
    const sessionId = `fixture-new-${createdSessions}`;
    createdSessionIds.push(sessionId);
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ session_id: sessionId }),
    });
  });

  await page.route("**/api/theme", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MOCK_THEME) }),
  );

  return { createdSessionIds };
}

async function navigateRunnerTest(page: Page) {
  await page.goto("/");
  await page.evaluate(() => sessionStorage.setItem("opman_token", "mock-jwt-token"));
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });
}

async function chooseRunnerSettings(page: Page, testCase: RunnerCase) {
  await chooseRunner(page, testCase.runner);
}

async function chooseCheapestModel(page: Page, testCase: RunnerCase) {
  // The runner's models are listed in the same palette the runner was chosen
  // in — that adjacency is the point of the merged control.
  const palette = page.locator('[role="dialog"][aria-label="Choose runner, model, and agent"]');
  await expect(palette.getByRole("option", { name: testCase.model }).first()).toBeVisible();
  await chooseModel(page, testCase.model);
  await expect(palette).not.toBeVisible();
}

/**
 * Send one prompt and check the whole request, not just its text.
 *
 * `runner` is deliberately asymmetric. The first prompt of a session carries it
 * — that is what binds the session to the runner the user chose. Later prompts
 * must not: the session is already bound, and naming a runner again is how
 * opman is asked to *switch* runners, which forks the conversation into a
 * handoff session. Model, effort, and permission belong on every turn.
 */
async function sendAndAssert(
  page: Page,
  testCase: RunnerCase,
  text: string,
  expect_: { sessionId: string; carriesRunner: boolean },
) {
  const requestPromise = page.waitForRequest((request) => {
    return request.method() === "POST" && new URL(request.url()).pathname.endsWith("/message");
  });
  const textarea = page.locator(".prompt-textarea");
  await textarea.fill(text);
  await textarea.press("Enter");
  const request = await requestPromise;
  const body = request.postDataJSON();

  expect(new URL(request.url()).pathname).toBe(`/api/session/${expect_.sessionId}/message`);
  expect(body.parts).toEqual([{ type: "text", text }]);
  if (expect_.carriesRunner) {
    expect(body.runner).toBe(testCase.runner);
  } else {
    expect(body).not.toHaveProperty("runner");
  }
  expect(body.model).toEqual({ providerID: testCase.provider, modelID: testCase.model });
  expect(body.effort).toBe("low");
  expect(body.permission).toBe(testCase.permission);
  await expect(textarea).toBeEnabled({ timeout: 10_000 });
}

test.describe("New session runner configuration", () => {
  // One test per runner: the shared loop needed four full configure-and-send
  // cycles inside a single 30s budget and timed out before it could assert
  // anything. Split, each case gets its own budget and its own failure.
  for (const testCase of RUNNERS) {
    test(`${testCase.runner}: configures the session, then keeps the follow-up in it`, async ({ page }) => {
      test.setTimeout(90_000);
      const fixtures = await installRunnerFixtures(page);
      await navigateRunnerTest(page);
      await expect(page.locator(".chat-layout")).toBeVisible();

      await page.locator(".sb-new-btn").click();
      await expect(page).toHaveURL(/new=1/);
      await chooseRunnerSettings(page, testCase);
      await chooseCheapestModel(page, testCase);
      await setEngineSettings(page, { effort: "low", permission: testCase.permission });
      await closeEnginePalette(page);

      // The first prompt creates the session for the chosen runner...
      const sessionId = "fixture-new-1";
      await sendAndAssert(page, testCase, `${testCase.runner} first message`, {
        sessionId, carriesRunner: true,
      });
      expect(fixtures.createdSessionIds).toEqual([sessionId]);

      // ...and the follow-up continues it: same session, no second creation, and
      // no runner on the wire to be mistaken for a switch request.
      await sendAndAssert(page, testCase, `${testCase.runner} second message`, {
        sessionId, carriesRunner: false,
      });
      expect(fixtures.createdSessionIds).toEqual([sessionId]);
    });
  }
});
