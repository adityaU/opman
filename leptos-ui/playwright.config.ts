import { defineConfig } from "@playwright/test";

/**
 * Playwright config for Leptos UI parity tests.
 *
 * Unlike the React suite (which spins up a Vite dev server), the Leptos UI is
 * served from the live opman backend at /ui. The OPMAN_PORT env var tells us
 * which port to hit; it defaults to 39633 for the current dev instance.
 */
const port = process.env.OPMAN_PORT ?? "39633";

export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  retries: 0,
  use: {
    baseURL: `http://localhost:${port}`,
    headless: true,
    serviceWorkers: "block",
  },
  // No webServer block — we rely on the running opman binary.
  projects: [
    {
      name: "chromium",
      use: { browserName: "chromium" },
    },
  ],
});
