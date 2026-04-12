import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  retries: 0,
  use: {
    baseURL: "http://localhost:5199",
    headless: true,
    // Block service workers so they cannot intercept fetch requests and
    // bypass Playwright route handlers used for API mocking in tests.
    serviceWorkers: "block",
  },
  webServer: {
    command: "npx vite --port 5199 --strictPort",
    port: 5199,
    reuseExistingServer: true,
    timeout: 15_000,
  },
  projects: [
    {
      name: "chromium",
      use: { browserName: "chromium" },
    },
  ],
});
