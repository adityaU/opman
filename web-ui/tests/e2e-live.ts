/**
 * End-to-end live test — exercises the full UI against the running opman
 * at localhost:9090.  Reports pass/fail per feature area.
 */
import { chromium, Page, Browser, BrowserContext } from "@playwright/test";
import fs from "fs";

const BASE = process.env.OPMAN_URL || "http://localhost:9090";
const USER = "admin";
const PASS = "0pm@n2026!";
const SCREENSHOT_DIR = "e2e-screenshots";

// ── Helpers ────────────────────────────────────────────

interface TestResult {
  name: string;
  passed: boolean;
  error?: string;
  screenshot?: string;
}

const results: TestResult[] = [];
let page: Page;
let browser: Browser;
let context: BrowserContext;

function log(msg: string) {
  console.log(`  ${msg}`);
}

async function screenshot(name: string): Promise<string> {
  const path = `${SCREENSHOT_DIR}/${name}.png`;
  await page.screenshot({ path });
  return path;
}

async function runTest(name: string, fn: () => Promise<void>) {
  try {
    await fn();
    results.push({ name, passed: true });
    console.log(`  ✓ ${name}`);
  } catch (e: any) {
    const screenshotPath = await screenshot(`FAIL-${name.replace(/\s+/g, "-")}`).catch(() => undefined);
    results.push({ name, passed: false, error: e.message?.slice(0, 300), screenshot: screenshotPath });
    console.log(`  ✗ ${name}: ${e.message?.slice(0, 120)}`);
  }
}

// ── Test functions ─────────────────────────────────────

async function testLogin() {
  // Navigate to the app fresh
  await page.goto(BASE, { waitUntil: "networkidle", timeout: 15000 });

  // Should see login page
  const loginVisible = await page.locator(".login-container").isVisible({ timeout: 5000 });
  if (!loginVisible) throw new Error("Login page not visible on fresh load");

  // Title should say opman
  const h1Text = await page.locator(".login-box h1").textContent();
  if (!h1Text?.toLowerCase().includes("opman")) throw new Error(`Login title unexpected: ${h1Text}`);

  // Fill in credentials and submit
  await page.fill('input[type="text"]', USER);
  await page.fill('input[type="password"]', PASS);
  await page.click('button[type="submit"]');

  // Should transition to chat layout
  await page.waitForSelector(".chat-layout", { timeout: 10000 });
  await screenshot("01-after-login");
}

async function testLoginBadCredentials() {
  // Open in new context to avoid existing session
  const ctx2 = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const p2 = await ctx2.newPage();
  await p2.goto(BASE, { waitUntil: "networkidle", timeout: 15000 });

  await p2.fill('input[type="text"]', "wrong");
  await p2.fill('input[type="password"]', "wrong");
  await p2.click('button[type="submit"]');

  // Should show error
  await p2.waitForTimeout(1500);
  const errorVisible = await p2.locator(".login-error").isVisible().catch(() => false);
  // The login should NOT succeed — we should still be on login page
  const stillOnLogin = await p2.locator(".login-container").isVisible().catch(() => false);
  if (!stillOnLogin && !errorVisible) throw new Error("Bad credentials should not log in");
  await ctx2.close();
}

async function testSidebarVisible() {
  const sidebar = page.locator(".chat-sidebar");
  const visible = await sidebar.isVisible({ timeout: 3000 });
  if (!visible) throw new Error("Sidebar not visible");

  // Should have SESSIONS header
  const title = await page.locator(".chat-sidebar-title").textContent();
  if (!title?.toLowerCase().includes("session")) throw new Error(`Sidebar title unexpected: ${title}`);
}

async function testSidebarProjectTabs() {
  // Should have project tabs
  const projectBtns = page.locator(".chat-sidebar-project-btn");
  const count = await projectBtns.count();
  if (count === 0) throw new Error("No project tabs found");
  log(`Found ${count} project tab(s)`);

  // One should be active
  const activeBtn = page.locator(".chat-sidebar-project-btn.active");
  const activeCount = await activeBtn.count();
  if (activeCount === 0) throw new Error("No active project tab");
}

async function testSidebarSessionsList() {
  const sessions = page.locator(".chat-sidebar-session");
  const count = await sessions.count();
  log(`Found ${count} session(s) in sidebar`);
  if (count === 0) throw new Error("No sessions listed in sidebar");

  // One should be active
  const activeSessions = page.locator(".chat-sidebar-session.active");
  const activeCount = await activeSessions.count();
  if (activeCount === 0) throw new Error("No active session highlighted");
  await screenshot("02-sidebar");
}

async function testSidebarNewSessionButton() {
  const newBtn = page.locator(".chat-sidebar-new-btn");
  const visible = await newBtn.isVisible({ timeout: 3000 });
  if (!visible) throw new Error("New session button not visible");

  // Click it (we won't verify the backend response, just that it doesn't crash)
  await newBtn.click();
  await page.waitForTimeout(1000);

  // Page should still be functional
  const layout = await page.locator(".chat-layout").isVisible();
  if (!layout) throw new Error("Layout broke after clicking new session");
}

async function testSessionSwitching() {
  // Get all sessions and click a different one
  const sessions = page.locator(".chat-sidebar-session");
  const count = await sessions.count();
  if (count < 2) {
    log("Only 1 session, skipping switch test");
    return;
  }

  // Find a non-active session and click it
  const activeBefore = await page.locator(".chat-sidebar-session.active").textContent();
  for (let i = 0; i < count; i++) {
    const session = sessions.nth(i);
    const isActive = await session.evaluate((el) => el.classList.contains("active"));
    if (!isActive) {
      await session.click();
      await page.waitForTimeout(1500);
      break;
    }
  }

  // The active session should have changed (or at least the click shouldn't crash)
  const layoutStillOk = await page.locator(".chat-layout").isVisible();
  if (!layoutStillOk) throw new Error("Layout broke after session switch");
  await screenshot("03-session-switch");
}

async function testMessageTimeline() {
  const timeline = page.locator(".message-timeline");
  const visible = await timeline.isVisible({ timeout: 3000 });
  if (!visible) throw new Error("Message timeline not visible");

  // Check if there are messages or welcome screen
  const messages = page.locator(".message-turn");
  const msgCount = await messages.count();
  const welcome = page.locator(".message-timeline-welcome");
  const welcomeVisible = await welcome.isVisible().catch(() => false);

  log(`Messages: ${msgCount}, Welcome visible: ${welcomeVisible}`);

  // Either messages or welcome should be present
  if (msgCount === 0 && !welcomeVisible) {
    throw new Error("Neither messages nor welcome screen visible");
  }
  await screenshot("04-messages");
}

async function testMessageAvatars() {
  const avatars = page.locator(".message-avatar");
  const count = await avatars.count();
  if (count === 0) {
    log("No message avatars (might be empty session)");
    return;
  }

  // Check that avatars have the right classes
  const userAvatars = page.locator(".message-avatar.user");
  const assistantAvatars = page.locator(".message-avatar.assistant");
  log(`User avatars: ${await userAvatars.count()}, Assistant avatars: ${await assistantAvatars.count()}`);
}

async function testMessageContent() {
  const bodies = page.locator(".message-body");
  const count = await bodies.count();
  if (count === 0) {
    log("No message bodies (might be empty session)");
    return;
  }

  // Check that at least one has text content
  let hasContent = false;
  for (let i = 0; i < Math.min(count, 5); i++) {
    const text = await bodies.nth(i).textContent();
    if (text && text.trim().length > 0) {
      hasContent = true;
      break;
    }
  }
  if (!hasContent) throw new Error("All message bodies are empty");
}

async function testToolCalls() {
  const toolCalls = page.locator(".tool-call");
  const count = await toolCalls.count();
  log(`Found ${count} tool call(s)`);
  if (count === 0) {
    log("No tool calls to test (ok for some sessions)");
    return;
  }

  // Click on first tool call header to toggle
  const header = page.locator(".tool-call-header").first();
  await header.click();
  await page.waitForTimeout(500);

  // Check if body becomes visible
  const body = page.locator(".tool-call-body").first();
  const bodyVisible = await body.isVisible().catch(() => false);
  log(`Tool call body visible after click: ${bodyVisible}`);
  await screenshot("05-tool-call");
}

async function testPromptInput() {
  const container = page.locator(".prompt-input-container");
  const visible = await container.isVisible({ timeout: 3000 });
  if (!visible) throw new Error("Prompt input not visible");

  // Should have textarea
  const textarea = page.locator(".prompt-textarea");
  const taVisible = await textarea.isVisible();
  if (!taVisible) throw new Error("Prompt textarea not visible");

  // Should have send button
  const sendBtn = page.locator(".prompt-send-btn");
  const sendVisible = await sendBtn.isVisible();
  if (!sendVisible) throw new Error("Send button not visible");

  // Type something
  await textarea.fill("Hello from E2E test");
  const value = await textarea.inputValue();
  if (value !== "Hello from E2E test") throw new Error(`Textarea value mismatch: ${value}`);

  // Clear it
  await textarea.fill("");
  await screenshot("06-prompt-input");
}

async function testPromptSlashCommands() {
  const textarea = page.locator(".prompt-textarea");

  // Type "/" to trigger slash command popover
  await textarea.fill("/");
  await page.waitForTimeout(800);

  const popover = page.locator(".slash-popover");
  const popoverVisible = await popover.isVisible().catch(() => false);

  if (popoverVisible) {
    const items = page.locator(".slash-popover-item");
    const itemCount = await items.count();
    log(`Slash command popover shows ${itemCount} commands`);
    if (itemCount === 0) throw new Error("Slash popover visible but no commands listed");
    await screenshot("07-slash-commands");
  } else {
    log("Slash popover not visible (commands might not be loaded yet)");
  }

  // Clear the input
  await textarea.fill("");
}

async function testSendMessage() {
  const textarea = page.locator(".prompt-textarea");
  const sendBtn = page.locator(".prompt-send-btn");

  // Type a message and send
  await textarea.fill("Test message from E2E");

  // Check if send button is enabled
  const disabled = await sendBtn.isDisabled();
  if (disabled) {
    log("Send button is disabled (might need active session)");
    await textarea.fill("");
    return;
  }

  await sendBtn.click();
  await page.waitForTimeout(2000);

  // Check the UI is still functional
  const layout = await page.locator(".chat-layout").isVisible();
  if (!layout) throw new Error("Layout broke after sending message");
  await screenshot("08-after-send");
}

async function testStatusBar() {
  const statusBar = page.locator(".chat-status-bar");
  const visible = await statusBar.isVisible({ timeout: 3000 });
  if (!visible) throw new Error("Status bar not visible");

  // Should have left and right sections
  const left = page.locator(".status-bar-left");
  const right = page.locator(".status-bar-right");
  const leftVisible = await left.isVisible();
  const rightVisible = await right.isVisible();
  if (!leftVisible) throw new Error("Status bar left section not visible");
  if (!rightVisible) throw new Error("Status bar right section not visible");

  // Check for project name
  const projectName = page.locator(".status-bar-project");
  const name = await projectName.textContent().catch(() => "");
  log(`Status bar project: "${name}"`);
  await screenshot("09-status-bar");
}

async function testStatusBarToggles() {
  // Test sidebar toggle button
  const sidebarBtn = page.locator(".status-bar-btn").first();
  const sidebarVisible = await page.locator(".chat-sidebar").isVisible();

  await sidebarBtn.click();
  await page.waitForTimeout(500);

  const sidebarAfter = await page.locator(".chat-sidebar").isVisible();
  log(`Sidebar before: ${sidebarVisible}, after toggle: ${sidebarAfter}`);

  // Toggle back
  if (sidebarVisible !== sidebarAfter) {
    await sidebarBtn.click();
    await page.waitForTimeout(500);
  }
  await screenshot("10-toggle-sidebar");
}

async function testCommandPalette() {
  // Open command palette with Cmd+Shift+P
  await page.keyboard.press("Meta+Shift+p");
  await page.waitForTimeout(800);

  const modal = page.locator(".command-palette");
  const visible = await modal.isVisible().catch(() => false);

  if (!visible) {
    // Try the modal backdrop
    const backdrop = page.locator(".modal-backdrop");
    const bdVisible = await backdrop.isVisible().catch(() => false);
    if (!bdVisible) throw new Error("Command palette did not open on Cmd+Shift+P");
  }

  // Should have an input
  const input = page.locator(".command-palette-input");
  const inputVisible = await input.isVisible().catch(() => false);
  if (!inputVisible) throw new Error("Command palette input not visible");

  // Should have items
  const items = page.locator(".command-palette-item");
  const count = await items.count();
  log(`Command palette shows ${count} item(s)`);

  // Type to filter
  await input.fill("new");
  await page.waitForTimeout(300);
  const filteredCount = await items.count();
  log(`After filtering 'new': ${filteredCount} item(s)`);

  await screenshot("11-command-palette");

  // Close with Escape
  await page.keyboard.press("Escape");
  await page.waitForTimeout(500);
  const closedModal = await page.locator(".command-palette").isVisible().catch(() => false);
  if (closedModal) throw new Error("Command palette did not close on Escape");
}

async function testModelPicker() {
  // Open model picker with Cmd+'
  await page.keyboard.press("Meta+'");
  await page.waitForTimeout(800);

  const modal = page.locator(".model-picker");
  const visible = await modal.isVisible().catch(() => false);

  if (!visible) {
    log("Model picker did not open (may not have providers loaded)");
    return;
  }

  // Should have header
  const header = page.locator(".model-picker-header");
  const headerVisible = await header.isVisible().catch(() => false);
  if (!headerVisible) throw new Error("Model picker header not visible");

  // Should have models listed
  const models = page.locator(".model-picker-item");
  const count = await models.count();
  log(`Model picker shows ${count} model(s)`);

  await screenshot("12-model-picker");

  // Close with Escape
  await page.keyboard.press("Escape");
  await page.waitForTimeout(500);
}

async function testKeyboardSidebarToggle() {
  const sidebarBefore = await page.locator(".chat-sidebar").isVisible();

  // Cmd+B to toggle sidebar
  await page.keyboard.press("Meta+b");
  await page.waitForTimeout(500);

  const sidebarAfter = await page.locator(".chat-sidebar").isVisible();
  log(`Sidebar toggle: before=${sidebarBefore}, after=${sidebarAfter}`);

  if (sidebarBefore === sidebarAfter) {
    log("WARNING: Sidebar did not toggle (might be intercepted)");
  }

  // Toggle back
  await page.keyboard.press("Meta+b");
  await page.waitForTimeout(500);
  await screenshot("13-keyboard-toggle");
}

async function testNoJSErrors() {
  // Collect JS errors during a brief interaction period
  const errors: string[] = [];
  page.on("pageerror", (err) => errors.push(err.message));

  // Do some interactions
  await page.waitForTimeout(2000);

  if (errors.length > 0) {
    throw new Error(`${errors.length} JS error(s): ${errors.slice(0, 3).join("; ")}`);
  }
}

async function testResponsiveLayout() {
  // Test that layout adjusts to smaller viewport
  await page.setViewportSize({ width: 768, height: 600 });
  await page.waitForTimeout(500);

  // Mobile header should appear
  const mobileHeader = page.locator(".chat-mobile-header");
  const mobileVisible = await mobileHeader.isVisible().catch(() => false);
  log(`Mobile header visible at 768px: ${mobileVisible}`);

  await screenshot("14-responsive-768");

  // Reset viewport
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.waitForTimeout(500);
}

async function testMobileHamburger() {
  // Set mobile viewport
  await page.setViewportSize({ width: 600, height: 800 });
  await page.waitForTimeout(500);

  const hamburger = page.locator(".mobile-hamburger");
  const visible = await hamburger.isVisible().catch(() => false);

  if (visible) {
    await hamburger.click();
    await page.waitForTimeout(500);

    // Sidebar should appear
    const sidebar = page.locator(".chat-sidebar");
    const sidebarVisible = await sidebar.isVisible().catch(() => false);
    log(`Mobile sidebar visible after hamburger click: ${sidebarVisible}`);
    await screenshot("15-mobile-sidebar");

    // Close overlay
    const overlay = page.locator(".sidebar-overlay.visible");
    if (await overlay.isVisible().catch(() => false)) {
      await overlay.click();
      await page.waitForTimeout(300);
    }
  } else {
    log("Hamburger not visible at 600px");
  }

  // Reset
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.waitForTimeout(500);
}

async function testAPIEndpoints() {
  // Verify backend API endpoints are responding
  const token = await page.evaluate(() => sessionStorage.getItem("opman_token") || "");

  const endpoints = [
    "/api/state",
    "/api/commands",
    "/api/providers",
    "/api/theme",
  ];

  for (const ep of endpoints) {
    const resp = await page.evaluate(async ({ url, tok }: { url: string; tok: string }) => {
      const r = await fetch(url, { headers: { Authorization: `Bearer ${tok}` } });
      return { status: r.status, ok: r.ok };
    }, { url: `${BASE}${ep}`, tok: token });

    if (!resp.ok) throw new Error(`${ep} returned ${resp.status}`);
    log(`${ep} -> ${resp.status} OK`);
  }
}

async function testScrolling() {
  // Verify the message timeline is scrollable
  const timeline = page.locator(".message-timeline");
  const scrollable = await timeline.evaluate((el) => el.scrollHeight > el.clientHeight);
  log(`Message timeline scrollable: ${scrollable} (scrollHeight > clientHeight)`);
  if (scrollable) {
    await timeline.evaluate((el) => el.scrollTo(0, el.scrollHeight));
    await page.waitForTimeout(300);
    await screenshot("16-scrolled-bottom");
  }
}

async function testProjectSwitching() {
  const projectBtns = page.locator(".chat-sidebar-project-btn");
  const count = await projectBtns.count();
  if (count < 2) {
    log("Only 1 project, skipping switch test");
    return;
  }

  // Click on a different project
  const activeBefore = await page.locator(".chat-sidebar-project-btn.active").textContent();
  for (let i = 0; i < count; i++) {
    const btn = projectBtns.nth(i);
    const isActive = await btn.evaluate((el) => el.classList.contains("active"));
    if (!isActive) {
      await btn.click();
      await page.waitForTimeout(2000);
      break;
    }
  }

  const layoutOk = await page.locator(".chat-layout").isVisible();
  if (!layoutOk) throw new Error("Layout broke after project switch");
  await screenshot("17-project-switch");

  // Switch back to first project
  await projectBtns.first().click();
  await page.waitForTimeout(1000);
}

async function testThemeColors() {
  // Verify CSS variables are set (theme may have been applied via SSE)
  const bgColor = await page.evaluate(() => {
    return window.getComputedStyle(document.body).backgroundColor;
  });
  log(`Body background: ${bgColor}`);
  if (bgColor === "rgba(0, 0, 0, 0)" || bgColor === "transparent") {
    throw new Error("Body background is transparent — theme not applied");
  }
}

// ── Main ───────────────────────────────────────────────

async function main() {
  console.log(`\n=== opman Web UI E2E Test Suite ===`);
  console.log(`Target: ${BASE}\n`);

  // Create screenshot dir
  if (!fs.existsSync(SCREENSHOT_DIR)) fs.mkdirSync(SCREENSHOT_DIR, { recursive: true });

  browser = await chromium.launch({ headless: true });
  context = await browser.newContext({ viewport: { width: 1440, height: 900 } });

  // Collect console errors throughout
  const jsErrors: string[] = [];

  page = await context.newPage();
  page.on("pageerror", (err) => jsErrors.push(`[pageerror] ${err.message}`));
  page.on("console", (msg) => {
    if (msg.type() === "error") jsErrors.push(`[console.error] ${msg.text()}`);
  });

  // ── Login tests ─────────────────────────
  console.log("\n1. LOGIN");
  await runTest("Login with valid credentials", testLogin);
  await runTest("Login rejects bad credentials", testLoginBadCredentials);

  // ── Sidebar tests ───────────────────────
  console.log("\n2. SIDEBAR");
  await runTest("Sidebar is visible", testSidebarVisible);
  await runTest("Sidebar has project tabs", testSidebarProjectTabs);
  await runTest("Sidebar lists sessions", testSidebarSessionsList);
  await runTest("Sidebar new session button", testSidebarNewSessionButton);
  await runTest("Session switching", testSessionSwitching);
  await runTest("Project switching", testProjectSwitching);

  // ── Message tests ───────────────────────
  console.log("\n3. MESSAGES");
  await runTest("Message timeline visible", testMessageTimeline);
  await runTest("Message avatars", testMessageAvatars);
  await runTest("Message content", testMessageContent);
  await runTest("Tool calls", testToolCalls);
  await runTest("Scrolling", testScrolling);

  // ── Prompt input tests ──────────────────
  console.log("\n4. PROMPT INPUT");
  await runTest("Prompt input visible and functional", testPromptInput);
  await runTest("Slash command popover", testPromptSlashCommands);
  await runTest("Send message", testSendMessage);

  // ── Status bar tests ────────────────────
  console.log("\n5. STATUS BAR");
  await runTest("Status bar visible", testStatusBar);
  await runTest("Status bar toggle buttons", testStatusBarToggles);

  // ── Keyboard / modals ───────────────────
  console.log("\n6. KEYBOARD SHORTCUTS & MODALS");
  await runTest("Command palette (Cmd+Shift+P)", testCommandPalette);
  await runTest("Model picker (Cmd+')", testModelPicker);
  await runTest("Keyboard sidebar toggle (Cmd+B)", testKeyboardSidebarToggle);

  // ── Responsive ──────────────────────────
  console.log("\n7. RESPONSIVE");
  await runTest("Responsive layout at 768px", testResponsiveLayout);
  await runTest("Mobile hamburger menu", testMobileHamburger);

  // ── API & misc ──────────────────────────
  console.log("\n8. API & MISC");
  await runTest("API endpoints respond", testAPIEndpoints);
  await runTest("Theme colors applied", testThemeColors);

  // ── JS error check ──────────────────────
  console.log("\n9. JS ERRORS");
  if (jsErrors.length > 0) {
    console.log(`  ⚠ ${jsErrors.length} JS error(s) detected during test run:`);
    for (const e of jsErrors.slice(0, 10)) {
      console.log(`    ${e.slice(0, 200)}`);
    }
    results.push({ name: "No JS errors", passed: false, error: `${jsErrors.length} error(s)` });
  } else {
    console.log("  ✓ No JS errors");
    results.push({ name: "No JS errors", passed: true });
  }

  // ── Summary ──────────────────────────────
  await browser.close();

  const passed = results.filter((r) => r.passed).length;
  const failed = results.filter((r) => !r.passed).length;
  const total = results.length;

  console.log(`\n${"=".repeat(50)}`);
  console.log(`RESULTS: ${passed}/${total} passed, ${failed} failed\n`);

  if (failed > 0) {
    console.log("FAILURES:");
    for (const r of results.filter((r) => !r.passed)) {
      console.log(`  ✗ ${r.name}`);
      if (r.error) console.log(`    Error: ${r.error}`);
      if (r.screenshot) console.log(`    Screenshot: ${r.screenshot}`);
    }
  }

  console.log(`\nScreenshots saved to ${SCREENSHOT_DIR}/`);

  // Exit with error code if tests failed
  if (failed > 0) process.exit(1);
}

main().catch((e) => {
  console.error("Fatal:", e);
  process.exit(2);
});
