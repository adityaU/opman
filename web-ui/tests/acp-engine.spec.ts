/**
 * ACP engine regressions.
 *
 * `claude -p` was replaced by a generic ACP engine: any agent declared in
 * config becomes its own runner. Three things broke or had to be invented for
 * that, and each has a test here:
 *
 *  - models were fetched without saying which runner they were for, so the
 *    claude runner showed an empty model list ("models are not coming");
 *  - `/api/agents` was proxied to whichever engine was primary, so selecting
 *    claude listed opencode's agents ("agents are from opencode not claude");
 *  - permission modes were a hardcoded per-runner table in the frontend, which
 *    a config-declared runner can never appear in, so they now ride on the
 *    provider payload.
 *
 * The last test is the point of the whole exercise: a runner name that exists
 * in no frontend table at all must work on configuration alone.
 */
import { test, expect, type Page } from "@playwright/test";
import { MOCK_APP_STATE, setupMockAPI } from "./helpers";
import { chooseRunner, openEnginePalette } from "./enginePicker";

const PALETTE = '[role="dialog"][aria-label="Choose runner, model, and agent"]';

/** Recorded when a request arrives without a `runner` param, so a revert is visible. */
const NO_RUNNER = "(none)";

type ProviderFixture = {
  providerId: string;
  providerName: string;
  models: Record<string, string>;
  permissionModes?: { value: string; label: string; description?: string }[];
};

const PROVIDERS: Record<string, ProviderFixture> = {
  opencode: {
    providerId: "fixture-opencode",
    providerName: "OpenCode Provider",
    models: { "qwen3-coder-free": "Qwen3 Coder" },
  },
  claude: {
    providerId: "anthropic",
    providerName: "Anthropic",
    models: { "opus[1m]": "Opus (1M context)", sonnet: "Sonnet" },
    // Deliberately *not* the labels in EngineSettingsRow's hardcoded table:
    // if the frontend ignored the payload these would never render.
    permissionModes: [
      { value: "bypassPermissions", label: "Bypass Permissions" },
      { value: "plan", label: "Plan Mode" },
    ],
  },
  codex: {
    providerId: "openai",
    providerName: "OpenAI",
    models: { "gpt-5-codex": "GPT-5 Codex" },
  },
  // A runner that appears in no hardcoded frontend table (not in RUNNER_LABELS,
  // not in the permission table, not in RUNNER_AGENT_FALLBACKS) — it exists
  // only because the backend declared it.
  gemini: {
    providerId: "google",
    providerName: "Google",
    models: { "gemini-3-pro": "Gemini 3 Pro", "gemini-3-flash": "Gemini 3 Flash" },
    permissionModes: [
      { value: "yolo", label: "YOLO Mode" },
      { value: "confirm-always", label: "Confirm Every Tool" },
    ],
  },
};

/** Agents as the *backend* reports them. ACP runners have no agent concept → []. */
const AGENTS: Record<string, unknown[]> = {
  opencode: [
    { id: "build", label: "Build", description: "Default coding agent", mode: "primary" },
    { id: "plan", label: "Plan", description: "Planning and design agent", mode: "all" },
  ],
  claude: [],
  codex: [],
  gemini: [],
};

function providerPayload(runner: string) {
  const fixture = PROVIDERS[runner] || PROVIDERS.opencode;
  const models: Record<string, unknown> = {};
  for (const [id, name] of Object.entries(fixture.models)) {
    models[id] = { id, name, limit: { context: 200000, output: 64000 } };
  }
  return {
    all: [{ id: fixture.providerId, name: fixture.providerName, models }],
    connected: [fixture.providerId],
    default: { [fixture.providerId]: Object.keys(fixture.models)[0] },
    ...(fixture.permissionModes ? { permissionModes: fixture.permissionModes } : {}),
  };
}

/** App state whose primary runner is opencode, so "scoped to claude" is falsifiable. */
function acpBackendState(runners: string[]) {
  return {
    ...MOCK_APP_STATE,
    backend: "opencode",
    default_runner: "opencode",
    runners,
    projects: [{ ...MOCK_APP_STATE.projects[0], active_session: null, sessions: [] }],
  };
}

async function installAcpFixtures(page: Page, runners = ["opencode", "claude", "codex"]) {
  await setupMockAPI(page);

  /** Every `runner` param the app asked with, in order. */
  const providerRunners: string[] = [];
  const agentRunners: string[] = [];

  await page.route("**/api/state", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(acpBackendState(runners)),
    }),
  );

  await page.route(/\/api\/providers(?:\?.*)?$/, (route) => {
    const runner = new URL(route.request().url()).searchParams.get("runner");
    providerRunners.push(runner ?? NO_RUNNER);
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(providerPayload(runner ?? "opencode")),
    });
  });

  await page.route(/\/api\/agents(?:\?.*)?$/, (route) => {
    const runner = new URL(route.request().url()).searchParams.get("runner");
    agentRunners.push(runner ?? NO_RUNNER);
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      // Without a runner param the old backend answered with the primary
      // engine's agents — reproduce that, so a revert shows opencode's rows.
      body: JSON.stringify(AGENTS[runner ?? "opencode"] ?? AGENTS.opencode),
    });
  });

  return { providerRunners, agentRunners };
}

async function login(page: Page) {
  await page.goto("/");
  await page.evaluate(() => sessionStorage.setItem("opman_token", "mock-jwt-token"));
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 15_000 });
}

/** Land on a fresh, unbound session — the only place a runner is still choosable. */
async function startNewSession(page: Page) {
  await page.locator(".sb-new-btn").click();
  await expect(page).toHaveURL(/new=1/);
}

async function permissionLabels(page: Page): Promise<string[]> {
  return page.locator(`${PALETTE} select[aria-label="Runner permissions"] option`).allTextContents();
}

test.describe("ACP engine: per-runner options", () => {
  test.describe.configure({ timeout: 60_000 });

  test("the claude runner lists its own models", async ({ page }) => {
    // The bug: providers were fetched unscoped, the claude runner's models never
    // arrived, and the palette's model group was empty.
    await installAcpFixtures(page);
    await login(page);
    await startNewSession(page);

    await chooseRunner(page, "claude");

    const palette = page.locator(PALETTE);
    await expect(palette.getByRole("option", { name: "Opus (1M context)" })).toBeVisible();
    await expect(palette.getByRole("option", { name: "Sonnet" })).toBeVisible();
    // And the previous runner's model is gone, so this is not just "any list".
    await expect(palette.getByRole("option", { name: "Qwen3 Coder" })).toHaveCount(0);
  });

  test("providers are requested per runner, not once globally", async ({ page }) => {
    const fixtures = await installAcpFixtures(page);
    await login(page);
    await startNewSession(page);

    // The startup fetch belongs to the primary runner...
    await expect.poll(() => fixtures.providerRunners).toContain("opencode");

    await chooseRunner(page, "claude");
    await expect.poll(() => fixtures.providerRunners).toContain("claude");

    // ...and each further switch asks again with its own scope, which is what
    // makes a config-declared runner able to have models at all.
    await chooseRunner(page, "codex");
    await expect.poll(() => fixtures.providerRunners).toContain("codex");

    expect(fixtures.providerRunners).not.toContain(NO_RUNNER);
  });

  test("agents are requested for the selected runner", async ({ page }) => {
    // The bug: /api/agents was proxied to the primary engine regardless of the
    // runner in hand, so choosing claude listed opencode's agents.
    const fixtures = await installAcpFixtures(page);
    await login(page);
    await startNewSession(page);

    await chooseRunner(page, "claude");

    await expect.poll(() => fixtures.agentRunners).toContain("claude");
    expect(fixtures.agentRunners).not.toContain(NO_RUNNER);
  });

  test("an ACP runner with no agents falls back to Default, not opencode's agents", async ({ page }) => {
    // ACP has no agent-selection concept, so the backend answers []. The client
    // must then show that runner's own single Default entry — the failure mode
    // being repaired is showing whatever the other engine happened to return.
    await installAcpFixtures(page);
    await login(page);
    await startNewSession(page);

    const palette = await openEnginePalette(page);
    // Prove opencode really does offer Build/Plan here, so the absence below
    // cannot pass just because agents never loaded.
    await expect(palette.getByRole("option", { name: "Default coding agent" })).toBeVisible();

    await chooseRunner(page, "claude");

    // RUNNER_AGENT_FALLBACKS.claude in src/api/session.ts
    await expect(palette.getByRole("option", { name: "Claude default agent" })).toBeVisible();
    await expect(palette.getByRole("option", { name: "Default coding agent" })).toHaveCount(0);
    await expect(palette.getByRole("option", { name: "Planning and design agent" })).toHaveCount(0);
  });

  test("permission modes come from the provider payload", async ({ page }) => {
    // Config-declared agents can never appear in a hardcoded per-runner table,
    // so the engine reports its modes and the table is only a backstop.
    await installAcpFixtures(page);
    await login(page);
    await startNewSession(page);

    await chooseRunner(page, "claude");

    await expect.poll(() => permissionLabels(page)).toEqual([
      "Bypass Permissions",
      "Plan Mode",
    ]);
    // The hardcoded fallback for "claude" would have produced these instead.
    expect(await permissionLabels(page)).not.toContain("Ask when needed");
    expect(await permissionLabels(page)).not.toContain("Auto-accept edits");
  });
});

test.describe("ACP engine: plug-and-play runners", () => {
  test("a runner known only to the backend gets its own models and permissions", async ({ page }) => {
    test.setTimeout(60_000);
    // "gemini" exists in no frontend table. Everything it shows must have come
    // over the wire — that is the whole promise of the config-driven engine.
    await installAcpFixtures(page, ["opencode", "claude", "gemini"]);
    await login(page);
    await startNewSession(page);

    const palette = await openEnginePalette(page);
    await expect(palette.getByRole("option", { name: "gemini", exact: true })).toBeVisible();

    await chooseRunner(page, "gemini");

    await expect(palette.getByRole("option", { name: "Gemini 3 Pro" })).toBeVisible();
    await expect(palette.getByRole("option", { name: "Gemini 3 Flash" })).toBeVisible();

    await expect.poll(() => permissionLabels(page)).toEqual([
      "YOLO Mode",
      "Confirm Every Tool",
    ]);

    // A runner absent from every frontend table used to fall through to *opencode's*
    // agent list, so a fresh ACP agent reporting no agents offered Build and Plan —
    // the same wrong-runner bug, one layer up. Unknown runners get a generic Default.
    // Matched on description: an agent row's accessible name carries it too.
    await expect(palette.getByRole("option", { name: "Default agent" })).toBeVisible();
    await expect(palette.getByRole("option", { name: "Default coding agent" })).toHaveCount(0);
    await expect(palette.getByRole("option", { name: "Planning and design agent" })).toHaveCount(0);
  });
});
