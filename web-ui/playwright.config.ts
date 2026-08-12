import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  retries: 0,
  workers: process.env.OPMAN_E2E_NVIM === "1" ? 1 : undefined,
  use: {
    baseURL: "http://localhost:5199",
    headless: true,
    // Block service workers so they cannot intercept fetch requests and
    // bypass Playwright route handlers used for API mocking in tests.
    serviceWorkers: "block",
  },
  webServer: {
    command: process.env.OPMAN_E2E_NVIM === "1"
      ? "node tests/nvim-editor/serve.mjs"
      : "npx vite --port 5199 --strictPort",
    port: 5199,
    reuseExistingServer: true,
    timeout: process.env.OPMAN_E2E_NVIM === "1" ? 60_000 : 15_000,
  },
  projects: [
    {
      name: "chromium",
      use: { browserName: "chromium" },
    },
  ],
});
