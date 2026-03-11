import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "pwa-install.spec.ts",
  timeout: 30_000,
  retries: 0,
  use: {
    baseURL: "http://localhost:5198",
    headless: true,
    // ALLOW service workers — this test validates SW registration
    serviceWorkers: "allow",
  },
  webServer: {
    // Serve the production build (dist/) on a different port
    command: "npx serve dist -l 5198 -s --no-clipboard",
    port: 5198,
    reuseExistingServer: true,
    timeout: 15_000,
    stdout: "pipe",
    stderr: "pipe",
  },
  projects: [
    {
      name: "chromium",
      use: { browserName: "chromium" },
    },
  ],
});
