/**
 * Visual debug — opens browser against the running opman at localhost:8080,
 * logs in, takes screenshots, and dumps layout info.
 */
import { chromium } from "@playwright/test";
import fs from "fs";

const BASE = process.env.OPMAN_URL || "http://localhost:9090";
const USER = "admin";
const PASS = "0pm@n2026!";

async function main() {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();

  const errors: string[] = [];
  page.on("pageerror", (err) => errors.push(err.message));
  page.on("console", (msg) => {
    if (msg.type() === "error") errors.push("[console.error] " + msg.text());
  });

  // ── Navigate ──────────────────────────────
  console.log("1. Navigating to", BASE);
  await page.goto(BASE, { waitUntil: "networkidle", timeout: 15000 });
  await page.screenshot({ path: "debug-01-initial.png", fullPage: true });
  console.log("   Saved debug-01-initial.png");

  // ── Login if needed ───────────────────────
  const isLogin = await page.locator(".login-container").isVisible().catch(() => false);
  if (isLogin) {
    console.log("2. On login page — logging in...");
    await page.fill('input[type="text"]', USER);
    await page.fill('input[type="password"]', PASS);
    await page.click('button[type="submit"]');
    // Wait for chat layout to appear
    try {
      await page.waitForSelector(".chat-layout", { timeout: 10000 });
      console.log("   Logged in successfully");
    } catch {
      console.log("   Login may have failed or chat-layout not found");
      await page.screenshot({ path: "debug-02-after-login-attempt.png", fullPage: true });
      console.log("   Saved debug-02-after-login-attempt.png");
    }
  }

  // ── Wait for rendering ────────────────────
  await page.waitForTimeout(3000);

  // ── Full page screenshot ──────────────────
  await page.screenshot({ path: "debug-03-full-page.png", fullPage: true });
  console.log("3. Saved debug-03-full-page.png");

  // ── Viewport screenshot (what user sees) ──
  await page.screenshot({ path: "debug-04-viewport.png" });
  console.log("4. Saved debug-04-viewport.png");

  // ── Sidebar screenshot ────────────────────
  const sidebar = page.locator(".chat-sidebar");
  if (await sidebar.isVisible().catch(() => false)) {
    await sidebar.screenshot({ path: "debug-05-sidebar.png" });
    console.log("5. Saved debug-05-sidebar.png");
    const sidebarHTML = await sidebar.innerHTML();
    fs.writeFileSync("debug-sidebar.html", sidebarHTML);
    console.log("   Saved debug-sidebar.html");
  } else {
    console.log("5. Sidebar NOT visible");
  }

  // ── Chat main screenshot ──────────────────
  const chatMain = page.locator(".chat-main");
  if (await chatMain.isVisible().catch(() => false)) {
    await chatMain.screenshot({ path: "debug-06-chat-main.png" });
    console.log("6. Saved debug-06-chat-main.png");
  } else {
    console.log("6. Chat main NOT visible");
  }

  // ── Status bar screenshot ─────────────────
  const statusBar = page.locator(".chat-status-bar");
  if (await statusBar.isVisible().catch(() => false)) {
    await statusBar.screenshot({ path: "debug-07-status-bar.png" });
    console.log("7. Saved debug-07-status-bar.png");
  } else {
    console.log("7. Status bar NOT visible");
  }

  // ── Computed styles for layout debugging ──
  const styles = await page.evaluate(() => {
    const results: Record<string, Record<string, string>> = {};
    const selectors = [
      "body",
      "#root",
      ".chat-layout",
      ".chat-content",
      ".chat-sidebar",
      ".chat-sidebar-header",
      ".chat-sidebar-sessions",
      ".chat-sidebar-session",
      ".chat-main",
      ".message-timeline",
      ".prompt-input-container",
      ".chat-status-bar",
      ".status-bar-left",
      ".status-bar-right",
    ];
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (el) {
        const cs = window.getComputedStyle(el);
        results[sel] = {
          display: cs.display,
          flexDirection: cs.flexDirection,
          position: cs.position,
          width: cs.width,
          height: cs.height,
          minWidth: cs.minWidth,
          maxWidth: cs.maxWidth,
          minHeight: cs.minHeight,
          overflow: cs.overflow,
          overflowX: cs.overflowX,
          overflowY: cs.overflowY,
          gridTemplateColumns: cs.gridTemplateColumns || "none",
          gridTemplateRows: cs.gridTemplateRows || "none",
          gap: cs.gap || "none",
          padding: cs.padding,
          margin: cs.margin,
          background: cs.backgroundColor,
          color: cs.color,
          border: cs.border,
          boxSizing: cs.boxSizing,
          flex: cs.flex,
        };
      } else {
        results[sel] = { error: "NOT FOUND in DOM" };
      }
    }
    return results;
  });
  fs.writeFileSync("debug-computed-styles.json", JSON.stringify(styles, null, 2));
  console.log("8. Saved debug-computed-styles.json");

  // ── CSS variables ─────────────────────────
  const cssVars = await page.evaluate(() => {
    const root = document.documentElement;
    const cs = window.getComputedStyle(root);
    const vars: Record<string, string> = {};
    const names = [
      "--color-bg", "--color-bg-panel", "--color-bg-element",
      "--color-text", "--color-text-muted", "--color-border",
      "--color-border-active", "--color-primary", "--color-accent",
      "--sidebar-width",
    ];
    for (const name of names) {
      vars[name] = cs.getPropertyValue(name) || "(not set)";
    }
    return vars;
  });
  fs.writeFileSync("debug-css-vars.json", JSON.stringify(cssVars, null, 2));
  console.log("   Saved debug-css-vars.json");

  // ── DOM tree (simplified) ─────────────────
  const domTree = await page.evaluate(() => {
    const lines: string[] = [];
    function walk(el: Element, depth: number) {
      if (depth > 6) return;
      const tag = el.tagName.toLowerCase();
      const cls = el.className && typeof el.className === "string"
        ? "." + el.className.trim().split(/\s+/).join(".")
        : "";
      const id = el.id ? "#" + el.id : "";
      const text = el.children.length === 0
        ? (el.textContent || "").trim().slice(0, 60)
        : "";
      const indent = "  ".repeat(depth);
      lines.push(indent + "<" + tag + id + cls + ">" + (text ? ' "' + text + '"' : ""));
      for (const child of Array.from(el.children)) {
        walk(child, depth + 1);
      }
    }
    const root = document.getElementById("root");
    if (root) walk(root, 0);
    return lines.join("\n");
  });
  fs.writeFileSync("debug-dom-tree.txt", domTree);
  console.log("9. Saved debug-dom-tree.txt");

  // ── Print errors ──────────────────────────
  if (errors.length > 0) {
    console.log("\n=== JS ERRORS (" + errors.length + ") ===");
    for (const e of errors) console.log("  " + e.slice(0, 200));
  } else {
    console.log("\nNo JS errors detected.");
  }

  await browser.close();
  console.log("\nDone. Check debug-*.png files for screenshots.");
}

main().catch((e) => {
  console.error("Fatal:", e);
  process.exit(1);
});
