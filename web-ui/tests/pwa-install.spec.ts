// PWA installability test — runs against the production build (dist/)
// Uses a real HTTP server (serve) and allows service workers.
//
// Run:  npx playwright test tests/pwa-install.spec.ts --config tests/pwa-install.config.ts

import { test, expect } from "@playwright/test";

test.describe("PWA installability", () => {
  test("manifest is served with correct content-type and fields", async ({ page }) => {
    const resp = await page.goto("/manifest.json");
    expect(resp).not.toBeNull();
    expect(resp!.status()).toBe(200);

    const ct = resp!.headers()["content-type"] || "";
    // Accept both application/json and application/manifest+json
    expect(ct).toMatch(/application\/(manifest\+)?json/);

    const manifest = await resp!.json();
    // Required fields for Chrome installability
    expect(manifest.name).toBeTruthy();
    expect(manifest.short_name).toBeTruthy();
    expect(manifest.start_url).toBeTruthy();
    expect(manifest.display).toMatch(/^(standalone|fullscreen|minimal-ui|window-controls-overlay)$/);

    // Must have 192 and 512 icons
    const sizes = (manifest.icons || []).map((i: any) => i.sizes);
    expect(sizes).toContain("192x192");
    expect(sizes).toContain("512x512");

    // Should not have prefer_related_applications: true
    expect(manifest.prefer_related_applications).not.toBe(true);

    console.log("Manifest parsed OK:", JSON.stringify(manifest, null, 2));
  });

  test("sw.js is served with correct content-type", async ({ page }) => {
    const resp = await page.goto("/sw.js");
    expect(resp).not.toBeNull();
    expect(resp!.status()).toBe(200);

    const ct = resp!.headers()["content-type"] || "";
    expect(ct).toContain("javascript");

    const body = await resp!.text();
    expect(body).toContain("addEventListener");
    expect(body).toContain("fetch");
    console.log("sw.js served OK, length:", body.length);
  });

  test("icons are served and are valid PNGs", async ({ page }) => {
    for (const icon of ["/icon-192.png", "/icon-512.png"]) {
      const resp = await page.goto(icon);
      expect(resp).not.toBeNull();
      expect(resp!.status()).toBe(200);
      const ct = resp!.headers()["content-type"] || "";
      expect(ct).toContain("image/png");
      const buf = await resp!.body();
      // PNG magic bytes
      expect(buf[0]).toBe(0x89);
      expect(buf[1]).toBe(0x50); // P
      expect(buf[2]).toBe(0x4e); // N
      expect(buf[3]).toBe(0x47); // G
      console.log(`${icon} OK, size: ${buf.length}`);
    }
  });

  test("service worker registers successfully", async ({ browser }) => {
    // Launch a context with service workers ALLOWED
    const context = await browser.newContext({
      serviceWorkers: "allow",
    });
    const page = await context.newPage();

    // Collect console messages
    const logs: string[] = [];
    page.on("console", (msg) => {
      logs.push(`[${msg.type()}] ${msg.text()}`);
    });

    await page.goto("/", { waitUntil: "load" });

    // Wait for SW registration log or timeout
    await page.waitForFunction(() => {
      return navigator.serviceWorker.controller !== null ||
        navigator.serviceWorker.ready !== undefined;
    }, { timeout: 10000 }).catch(() => {});

    // Give SW time to install + activate
    await page.waitForTimeout(3000);

    // Check SW registration state
    const swInfo = await page.evaluate(async () => {
      if (!("serviceWorker" in navigator)) {
        return { error: "serviceWorker not in navigator" };
      }
      try {
        const reg = await navigator.serviceWorker.getRegistration("/");
        if (!reg) {
          return { error: "no registration found for scope /" };
        }
        return {
          scope: reg.scope,
          installing: reg.installing ? reg.installing.state : null,
          waiting: reg.waiting ? reg.waiting.state : null,
          active: reg.active ? reg.active.state : null,
        };
      } catch (e: any) {
        return { error: e.message || String(e) };
      }
    });

    console.log("SW registration info:", JSON.stringify(swInfo, null, 2));
    console.log("Console logs:", logs.join("\n"));

    // The SW must be active
    expect(swInfo.error).toBeUndefined();
    expect(swInfo.active).toBe("activated");

    await context.close();
  });

  test("page has manifest link tag", async ({ page }) => {
    await page.goto("/", { waitUntil: "domcontentloaded" });
    const manifestLink = await page.$('link[rel="manifest"]');
    expect(manifestLink).not.toBeNull();
    const href = await manifestLink!.getAttribute("href");
    expect(href).toBe("/manifest.json");
    console.log("Manifest link found:", href);
  });

  test("full installability diagnostic", async ({ browser }) => {
    const context = await browser.newContext({
      serviceWorkers: "allow",
    });
    const page = await context.newPage();

    const logs: string[] = [];
    page.on("console", (msg) => logs.push(`[${msg.type()}] ${msg.text()}`));

    // Listen for beforeinstallprompt
    await page.goto("/", { waitUntil: "load" });

    // Wait for SW to activate
    await page.waitForTimeout(3000);

    const diagnostic = await page.evaluate(async () => {
      const result: Record<string, any> = {};

      // 1. Secure context?
      result.isSecureContext = window.isSecureContext;

      // 2. Manifest link present?
      const link = document.querySelector('link[rel="manifest"]');
      result.manifestLinkPresent = !!link;
      result.manifestHref = link?.getAttribute("href") || null;

      // 3. Fetch and parse manifest
      try {
        const resp = await fetch("/manifest.json");
        result.manifestStatus = resp.status;
        result.manifestContentType = resp.headers.get("content-type");
        const m = await resp.json();
        result.manifest = {
          hasName: !!m.name,
          hasShortName: !!m.short_name,
          hasStartUrl: !!m.start_url,
          display: m.display,
          hasIcons: Array.isArray(m.icons),
          iconSizes: (m.icons || []).map((i: any) => `${i.sizes}:${i.purpose}`),
          preferRelatedApps: m.prefer_related_applications,
          id: m.id,
          scope: m.scope,
        };
      } catch (e: any) {
        result.manifestError = e.message;
      }

      // 4. Service worker state
      if ("serviceWorker" in navigator) {
        result.swSupported = true;
        try {
          const reg = await navigator.serviceWorker.getRegistration("/");
          if (reg) {
            result.sw = {
              scope: reg.scope,
              installing: reg.installing?.state || null,
              waiting: reg.waiting?.state || null,
              active: reg.active?.state || null,
            };
          } else {
            result.sw = { error: "no registration at /" };
          }
        } catch (e: any) {
          result.sw = { error: e.message };
        }
      } else {
        result.swSupported = false;
      }

      // 5. Check if icons are actually fetchable
      result.iconChecks = {};
      for (const path of ["/icon-192.png", "/icon-512.png"]) {
        try {
          const r = await fetch(path);
          result.iconChecks[path] = {
            status: r.status,
            contentType: r.headers.get("content-type"),
            size: (await r.arrayBuffer()).byteLength,
          };
        } catch (e: any) {
          result.iconChecks[path] = { error: e.message };
        }
      }

      return result;
    });

    console.log("=== FULL PWA DIAGNOSTIC ===");
    console.log(JSON.stringify(diagnostic, null, 2));
    console.log("Console logs:", logs.join("\n"));

    // Assertions
    expect(diagnostic.isSecureContext).toBe(true);
    expect(diagnostic.manifestLinkPresent).toBe(true);
    expect(diagnostic.manifestStatus).toBe(200);
    expect(diagnostic.manifest.hasName).toBe(true);
    expect(diagnostic.manifest.hasShortName).toBe(true);
    expect(diagnostic.manifest.hasStartUrl).toBe(true);
    expect(diagnostic.manifest.display).toBe("standalone");
    expect(diagnostic.manifest.hasIcons).toBe(true);
    expect(diagnostic.swSupported).toBe(true);
    expect(diagnostic.sw.active).toBe("activated");
    expect(diagnostic.iconChecks["/icon-192.png"].status).toBe(200);
    expect(diagnostic.iconChecks["/icon-512.png"].status).toBe(200);

    await context.close();
  });
});
