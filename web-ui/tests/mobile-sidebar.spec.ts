/**
 * Mobile sidebar tests.
 *
 * Uses a 375×812 viewport (iPhone-sized) to verify sidebar behaviour on
 * mobile. Mocked API via helpers — no real backend needed.
 *
 * Key regression covered: the sidebar wrapper's `.panel-dimmed` class
 * (opacity:0.75) was creating a stacking context that caused the
 * position:fixed sidebar's background to not paint, making it transparent.
 * Fixed by forcing `opacity: 1 !important` on the wrapper in the mobile
 * media query.
 */

import { test, expect } from "@playwright/test";
import { navigateAuthenticated } from "./helpers";

const MOBILE_VIEWPORT = { width: 375, height: 812 };

test.describe("Mobile sidebar", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize(MOBILE_VIEWPORT);
    await navigateAuthenticated(page);
  });

  test("sidebar is off-screen by default", async ({ page }) => {
    const sidebar = page.locator(".chat-sidebar");
    await expect(sidebar).toBeAttached();

    const box = await sidebar.boundingBox();
    if (box) {
      expect(box.x + box.width).toBeLessThanOrEqual(0);
    }
  });

  test("status pill is visible on mobile", async ({ page }) => {
    await expect(page.locator(".mobile-status-pill")).toBeVisible();
  });

  test("clicking status pill opens sidebar on-screen", async ({ page }) => {
    await page.locator(".mobile-status-pill").click();

    const sidebar = page.locator(".chat-sidebar");
    await expect(sidebar).toHaveClass(/mobile-open/, { timeout: 5_000 });
    await page.waitForTimeout(500);

    const box = await sidebar.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.width).toBeGreaterThan(0);
  });

  test("sidebar wrapper has opacity 1 on mobile (regression: panel-dimmed fix)", async ({ page }) => {
    const wrapperOpacity = await page.evaluate(() => {
      const sidebar = document.querySelector(".chat-sidebar");
      const wrapper = sidebar?.parentElement;
      if (!wrapper) return "no-wrapper";
      return window.getComputedStyle(wrapper).opacity;
    });

    expect(wrapperOpacity).toBe("1");
  });

  test("sidebar is opaque — no ancestor has opacity < 1", async ({ page }) => {
    await page.locator(".mobile-status-pill").click();
    await page.locator(".chat-sidebar").waitFor({ state: "visible" });
    await page.waitForTimeout(600);

    // Verify sidebar itself has solid background
    const sidebarStyles = await page.locator(".chat-sidebar").evaluate((el) => {
      const cs = window.getComputedStyle(el);
      return {
        backgroundColor: cs.backgroundColor,
        opacity: cs.opacity,
      };
    });

    expect(sidebarStyles.opacity).toBe("1");
    expect(sidebarStyles.backgroundColor).not.toBe("rgba(0, 0, 0, 0)");
    expect(sidebarStyles.backgroundColor).not.toBe("transparent");

    // No ancestor may have opacity < 1 (this was the root cause)
    const ancestorOpacities = await page.locator(".chat-sidebar").evaluate((el) => {
      const result: Array<{ class: string; opacity: string }> = [];
      let node: HTMLElement | null = el.parentElement;
      while (node && node !== document.documentElement) {
        result.push({
          class: node.className.slice(0, 40),
          opacity: window.getComputedStyle(node).opacity,
        });
        node = node.parentElement;
      }
      return result;
    });

    for (const ancestor of ancestorOpacities) {
      expect(ancestor.opacity).toBe("1");
    }
  });

  test("sidebar has correct computed styles when open", async ({ page }) => {
    await page.locator(".mobile-status-pill").click();

    const sidebar = page.locator(".chat-sidebar");
    await expect(sidebar).toHaveClass(/mobile-open/, { timeout: 5_000 });
    await page.waitForTimeout(500);

    const styles = await sidebar.evaluate((el) => {
      const cs = window.getComputedStyle(el);
      return {
        display: cs.display,
        visibility: cs.visibility,
        opacity: cs.opacity,
        transform: cs.transform,
        position: cs.position,
        zIndex: cs.zIndex,
      };
    });

    expect(styles.display).not.toBe("none");
    expect(styles.visibility).not.toBe("hidden");
    expect(styles.opacity).not.toBe("0");
    expect(styles.position).toBe("fixed");
    expect(Number(styles.zIndex)).toBeGreaterThanOrEqual(70);
    // transform should be identity when translateX(0)
    expect(styles.transform).not.toContain("translateX");
  });

  test("sidebar overlay appears when sidebar is open", async ({ page }) => {
    await page.locator(".mobile-status-pill").click();
    await expect(page.locator(".sidebar-overlay")).toBeVisible({ timeout: 5_000 });
  });

  test("sidebar shows sessions and project name", async ({ page }) => {
    await page.locator(".mobile-status-pill").click();

    const sidebar = page.locator(".chat-sidebar");
    await expect(sidebar).toHaveClass(/mobile-open/, { timeout: 5_000 });
    await page.waitForTimeout(500);

    await expect(sidebar.locator(".sb-session-group")).toHaveCount(2);
    await expect(sidebar.getByText("Test Session")).toBeVisible();
    await expect(sidebar.getByText("Another Session")).toBeVisible();
    await expect(sidebar.getByText("my-project", { exact: false })).toBeVisible();
    await expect(sidebar.locator(".sb-new-btn")).toBeVisible();
  });

  test("sidebar wrapper has width:0 and overflow:visible on mobile", async ({ page }) => {
    const wrapperStyles = await page.locator(".chat-content > div:first-child").evaluate((el) => {
      const cs = window.getComputedStyle(el);
      return { width: cs.width, overflow: cs.overflow };
    });

    expect(wrapperStyles.width).toBe("0px");
    expect(wrapperStyles.overflow).toBe("visible");
  });

  test("clicking overlay closes sidebar", async ({ page }) => {
    await page.locator(".mobile-status-pill").click();
    await expect(page.locator(".chat-sidebar")).toHaveClass(/mobile-open/, { timeout: 5_000 });

    // Click the overlay on the right side of the viewport, outside the sidebar
    // (sidebar is max 300px wide on a 375px viewport, so right edge is clear)
    await page.locator(".sidebar-overlay").click({ position: { x: 350, y: 400 } });
    await page.waitForTimeout(500);

    await expect(page.locator(".chat-sidebar")).not.toHaveClass(/mobile-open/);
  });
});
