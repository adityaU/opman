/**
 * Leptos UI parity tests — Send message flow.
 * Mirrors: web-ui/tests/send-message.spec.ts
 */
import { test, expect } from "@playwright/test";
import { navigateAuthenticated, SESSION_ID } from "./helpers";

test.describe("Send message", () => {
  test.beforeEach(async ({ page }) => {
    await navigateAuthenticated(page);
  });

  test("typing and clicking send dispatches POST request", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await textarea.fill("What is the meaning of life?");

    const reqPromise = page.waitForRequest(
      (req) =>
        req.url().includes(`/api/session/${SESSION_ID}/message`) &&
        req.method() === "POST",
      { timeout: 5_000 }
    );

    await page.locator(".prompt-send-btn").click();
    const req = await reqPromise;
    const body = req.postDataJSON?.() ?? JSON.parse(req.postData() || "{}");
    expect(body.content || body.text || body.message).toContain("What is the meaning of life?");
  });

  test("pressing Enter sends the message", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await textarea.fill("Hello from Enter key");

    const reqPromise = page.waitForRequest(
      (req) =>
        req.url().includes(`/api/session/${SESSION_ID}/message`) &&
        req.method() === "POST",
      { timeout: 5_000 }
    );

    await textarea.press("Enter");
    await reqPromise; // resolves = request was made
  });

  test("Shift+Enter does NOT send the message", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    await textarea.fill("Line 1");

    let sent = false;
    page.on("request", (req) => {
      if (req.url().includes("/message") && req.method() === "POST") {
        sent = true;
      }
    });

    await textarea.press("Shift+Enter");
    await page.waitForTimeout(1000);
    expect(sent).toBe(false);
  });

  test("textarea placeholder is visible", async ({ page }) => {
    const textarea = page.locator(".prompt-textarea");
    const placeholder = await textarea.getAttribute("placeholder");
    expect(placeholder).toBeTruthy();
  });

  test("send button is a <button> element", async ({ page }) => {
    const tagName = await page.locator(".prompt-send-btn").evaluate(
      (el) => el.tagName.toLowerCase()
    );
    expect(tagName).toBe("button");
  });
});
