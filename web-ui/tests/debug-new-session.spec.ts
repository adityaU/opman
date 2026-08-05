import { test, expect, type Page } from "@playwright/test";
import { chooseRunner, closeEnginePalette } from "./enginePicker";

/** Diagnostic probe: watch the transcript DOM from the moment a new-session
 *  prompt is submitted. Not part of the normal suite. */
test.describe("debug new session", () => {
  test.skip(!process.env.OPMAN_DEBUG_NEW_SESSION, "diagnostic only");
  test.setTimeout(5 * 60 * 1000);

  async function login(page: Page) {
    await page.goto(process.env.OPMAN_REAL_URL || "http://127.0.0.1:9090", { waitUntil: "networkidle" });
    if (!await page.locator(".login-container").isVisible().catch(() => false)) return;
    await page.locator('input[type="text"]').fill(process.env.OPMAN_REAL_USER || "admin");
    await page.locator('input[type="password"]').fill(process.env.OPMAN_REAL_PASSWORD || "");
    await page.locator('button[type="submit"]').click();
    await page.waitForSelector(".chat-layout", { timeout: 180_000 });
  }

  test("probe", async ({ page }) => {
    page.on("console", (msg) => console.log(`  [console.${msg.type()}] ${msg.text().slice(0, 200)}`));
    await login(page);

    await page.locator(".sb-new-btn").click();
    await expect(page).toHaveURL(/new=1/);

    await chooseRunner(page, "claude");
    await closeEnginePalette(page);

    const marker = "DEBUG_PROBE_MARKER";
    await page.locator(".prompt-textarea").fill(`Reply with exactly ${marker}.`);
    await page.locator(".prompt-send-btn").click();

    for (let i = 0; i < 40; i++) {
      const snap = await page.evaluate(async () => {
        const state = await fetch("/api/state").then((r) => r.json()).catch(() => null);
        const proj = state?.projects?.[state.active_project];
        const busy = proj?.busy_sessions ?? [];
        const turns = [...document.querySelectorAll(".message-turn")].map((el) => ({
          cls: el.className,
          text: (el.textContent || "").slice(0, 40),
        }));
        return {
          url: location.search,
          turns,
          welcome: !!document.querySelector(".new-session-welcome"),
          pending: !!document.querySelector(".pending-reply"),
          shimmer: !!document.querySelector(".message-shimmer"),
          gate: !!document.querySelector(".startup-gate"),
          busy,
          activeBusy: busy.includes(proj?.active_session),
          sendBtn: document.querySelector(".prompt-send-btn")?.getAttribute("title"),
        };
      });
      console.log(`t=${(i * 0.5).toFixed(1)}s ${JSON.stringify(snap)}`);
      if (snap.turns.some((t) => t.text.includes(marker) && t.cls.includes("assistant"))) break;
      await page.waitForTimeout(500);
    }
  });
});
