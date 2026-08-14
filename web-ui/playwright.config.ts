import { defineConfig } from "@playwright/test";

const editorSuite = process.env.OPMAN_E2E_EDITOR === "1";

export default defineConfig({
  testDir: "./tests",
  // Every editor test boots the real backend page, logs in and waits for the
  // language server before its body runs. That setup is charged against the
  // test budget, so the hermetic suite gets a larger one.
  timeout: editorSuite ? 60_000 : 30_000,
  retries: 0,
  workers: editorSuite ? 1 : undefined,
  use: {
    baseURL: "http://localhost:5199",
    headless: true,
    // Block service workers so they cannot intercept fetch requests and
    // bypass Playwright route handlers used for API mocking in tests.
    serviceWorkers: "block",
  },
  webServer: {
    command: editorSuite
      ? "node tests/editor/serve.mjs"
      : "npx vite --port 5199 --strictPort",
    port: 5199,
    // The editor harness publishes a backend URL in `.state.json` alongside the
    // vite server it owns. Reusing a leftover harness pairs this run with a
    // backend the previous run is about to tear down, which reads as a mid-run
    // ECONNREFUSED cascade rather than as the stale server it is.
    reuseExistingServer: !editorSuite,
    timeout: editorSuite ? 60_000 : 15_000,
  },
  projects: [
    {
      name: "chromium",
      use: { browserName: "chromium" },
    },
  ],
});
