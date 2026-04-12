/**
 * Mobile input autohide tests.
 *
 * Uses a 375×812 viewport (iPhone-sized) to verify the input area hides
 * on scroll-up and on message send. Reveal is done exclusively via the
 * compose button in the mobile dock. Also covers the textarea-content
 * guard, small-scroll threshold, and desktop unaffected behavior.
 */

import { test, expect, Page } from "@playwright/test";
import { setupMockAPI, SESSION_ID, MOCK_APP_STATE, MOCK_STATS } from "./helpers";

const MOBILE_VIEWPORT = { width: 375, height: 812 };
const DESKTOP_VIEWPORT = { width: 1280, height: 800 };

/** Generate many mock messages so the timeline is scrollable */
function generateManyMessages(count: number) {
  const messages = [];
  for (let i = 0; i < count; i++) {
    const role = i % 2 === 0 ? "user" : "assistant";
    messages.push({
      info: {
        role,
        messageID: `msg_autohide_${String(i).padStart(3, "0")}`,
        time: 1700000000 + i * 100,
        ...(role === "assistant"
          ? { model: { modelID: "claude-sonnet-4-20250514", providerID: "anthropic" } }
          : {}),
      },
      parts: [
        {
          type: "text",
          text: `Message ${i}: ${"Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(3)}`,
        },
      ],
      metadata: role === "assistant" ? { cost: 0.001, tokens: { input: 10, output: 20, reasoning: 0, cache_read: 0, cache_write: 0 } } : {},
    });
  }
  return messages;
}

const MANY_MESSAGES = generateManyMessages(40);

async function navigateWithManyMessages(page: Page) {
  await setupMockAPI(page);

  // Override messages route with many messages for scrollable content
  await page.route(`**/api/session/${SESSION_ID}/messages*`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ messages: MANY_MESSAGES, total: MANY_MESSAGES.length }),
    })
  );

  await page.goto("/");
  await page.evaluate(() => {
    sessionStorage.setItem("opman_token", "mock-jwt-token");
  });
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });
}

/**
 * Scroll the message timeline by a given delta using mouse wheel events.
 * Negative delta = scroll up, positive = scroll down.
 */
async function scrollTimeline(page: Page, deltaY: number) {
  const timeline = page.locator(".message-timeline");
  const box = await timeline.boundingBox();
  if (!box) throw new Error("Timeline not found");

  // Use mouse.wheel which triggers real scroll behavior
  const centerX = box.x + box.width / 2;
  const centerY = box.y + box.height / 2;
  await page.mouse.move(centerX, centerY);

  // Dispatch wheel in multiple smaller steps to simulate real scrolling
  const steps = 8;
  const stepDelta = deltaY / steps;
  for (let i = 0; i < steps; i++) {
    await page.mouse.wheel(0, stepDelta);
    await page.waitForTimeout(30);
  }

  // Allow rAF + debounce (150ms) + React render to settle
  await page.waitForTimeout(500);
}

/** Helper: hide input via scroll-up, then reveal via compose button */
async function hideAndRevealViaCompose(page: Page) {
  const timeline = page.locator(".message-timeline");
  await timeline.evaluate((el) => { el.scrollTop = el.scrollHeight; });
  await page.waitForTimeout(300);
  await scrollTimeline(page, -200);
  await expect(page.locator(".mobile-input-wrapper")).toHaveClass(/mobile-input-hidden/, { timeout: 2_000 });
  await page.locator(".mobile-compose-btn").click();
  await page.waitForTimeout(200);
  await expect(page.locator(".mobile-input-wrapper")).not.toHaveClass(/mobile-input-hidden/, { timeout: 2_000 });
}

test.describe("Mobile input autohide", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize(MOBILE_VIEWPORT);
    await navigateWithManyMessages(page);
  });

  test("input wrapper is visible by default", async ({ page }) => {
    const wrapper = page.locator(".mobile-input-wrapper");
    await expect(wrapper).toBeAttached();
    await expect(wrapper).not.toHaveClass(/mobile-input-hidden/);
  });

  test("compose button is not visible when input is shown", async ({ page }) => {
    await expect(page.locator(".mobile-compose-btn")).not.toHaveClass(/visible/);
  });

  test("scrolling up hides the input area", async ({ page }) => {
    const timeline = page.locator(".message-timeline");

    // First scroll to the bottom so we have room to scroll up
    await timeline.evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await page.waitForTimeout(300);

    // Now scroll up by a large amount (well above 20px threshold)
    await scrollTimeline(page, -200);

    const wrapper = page.locator(".mobile-input-wrapper");
    await expect(wrapper).toHaveClass(/mobile-input-hidden/, { timeout: 2_000 });
  });

  test("scrolling down does NOT reveal the input area", async ({ page }) => {
    const timeline = page.locator(".message-timeline");

    // Scroll to bottom first
    await timeline.evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await page.waitForTimeout(300);

    // Scroll up to hide input
    await scrollTimeline(page, -200);
    await expect(page.locator(".mobile-input-wrapper")).toHaveClass(/mobile-input-hidden/, { timeout: 2_000 });

    // Scroll down — input should stay hidden (reveal only via compose button)
    await scrollTimeline(page, 200);
    await expect(page.locator(".mobile-input-wrapper")).toHaveClass(/mobile-input-hidden/);
  });

  test("sending a message hides input on mobile", async ({ page }) => {
    // Mock the send endpoint
    await page.route(`**/api/session/${SESSION_ID}/message`, (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: "{}" })
    );

    const textarea = page.locator(".prompt-textarea");
    await textarea.fill("hello world");
    await page.waitForTimeout(50);

    // Press Enter to send
    await textarea.press("Enter");

    // Input should be hidden after send on mobile
    const wrapper = page.locator(".mobile-input-wrapper");
    await expect(wrapper).toHaveClass(/mobile-input-hidden/, { timeout: 2_000 });

    // Compose button should appear
    await expect(page.locator(".mobile-compose-btn")).toHaveClass(/visible/, { timeout: 2_000 });
  });

  test("compose button appears when input is hidden", async ({ page }) => {
    const timeline = page.locator(".message-timeline");

    // Scroll to bottom then up
    await timeline.evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await page.waitForTimeout(300);
    await scrollTimeline(page, -200);

    await expect(page.locator(".mobile-input-wrapper")).toHaveClass(/mobile-input-hidden/, { timeout: 2_000 });
    await expect(page.locator(".mobile-compose-btn")).toHaveClass(/visible/, { timeout: 2_000 });
  });

  test("compose button tap reveals input and focuses textarea", async ({ page }) => {
    const timeline = page.locator(".message-timeline");

    // Hide input
    await timeline.evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await page.waitForTimeout(300);
    await scrollTimeline(page, -200);
    await expect(page.locator(".mobile-input-wrapper")).toHaveClass(/mobile-input-hidden/, { timeout: 2_000 });

    // Tap compose button
    await page.locator(".mobile-compose-btn").click();
    await page.waitForTimeout(200);

    // Input should be visible again
    await expect(page.locator(".mobile-input-wrapper")).not.toHaveClass(/mobile-input-hidden/, { timeout: 2_000 });

    // Compose button should disappear (no longer has "visible" class)
    await expect(page.locator(".mobile-compose-btn")).not.toHaveClass(/visible/, { timeout: 2_000 });
  });

  test("input does not hide when textarea has content", async ({ page }) => {
    // Type something in the textarea first
    const textarea = page.locator(".prompt-textarea");
    await textarea.fill("some text");
    await page.waitForTimeout(50);

    // Now try to scroll up
    const timeline = page.locator(".message-timeline");
    await timeline.evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await page.waitForTimeout(300);
    await scrollTimeline(page, -200);

    // Input should NOT be hidden — guard is active
    const wrapper = page.locator(".mobile-input-wrapper");
    await expect(wrapper).not.toHaveClass(/mobile-input-hidden/);
  });

  test("small scroll does not trigger hide", async ({ page }) => {
    const timeline = page.locator(".message-timeline");

    // ── Phase 1: normalize state with a full hide→reveal-via-compose cycle ──
    await timeline.evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await page.waitForTimeout(300);

    const box = await timeline.boundingBox();
    if (!box) throw new Error("Timeline not found");
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);

    await scrollTimeline(page, -100);
    await expect(page.locator(".mobile-input-wrapper")).toHaveClass(/mobile-input-hidden/, { timeout: 2_000 });

    // Reveal via compose button (scroll-down no longer reveals)
    await page.locator(".mobile-compose-btn").click();
    await page.waitForTimeout(200);
    await expect(page.locator(".mobile-input-wrapper")).not.toHaveClass(/mobile-input-hidden/, { timeout: 2_000 });

    // ── Phase 2: scroll to bottom to settle direction refs ──
    await scrollTimeline(page, 300);
    await page.waitForTimeout(400);

    // Confirm still visible (compose button revealed it, scroll-down doesn't affect)
    await expect(page.locator(".mobile-input-wrapper")).not.toHaveClass(/mobile-input-hidden/);

    // ── Phase 3: small scroll up (below 20px threshold) ──
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.wheel(0, -10);
    // Wait for rAF + debounce (150ms) + React render to settle
    await page.waitForTimeout(500);

    const wrapper = page.locator(".mobile-input-wrapper");
    await expect(wrapper).not.toHaveClass(/mobile-input-hidden/);
  });
});

test.describe("Desktop input unaffected", () => {
  test("input wrapper does not get hidden class on desktop", async ({ page }) => {
    await page.setViewportSize(DESKTOP_VIEWPORT);
    await navigateWithManyMessages(page);

    const timeline = page.locator(".message-timeline");

    // Scroll to bottom then up
    await timeline.evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await page.waitForTimeout(300);
    await scrollTimeline(page, -200);

    // On desktop, the wrapper should never get the hidden class
    // because handleScrollDirection gates on window.innerWidth >= 768
    const wrapper = page.locator(".mobile-input-wrapper");
    await expect(wrapper).not.toHaveClass(/mobile-input-hidden/);
  });
});
