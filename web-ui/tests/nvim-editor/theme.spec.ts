import { expect, gateNvimE2E, nvimTest } from "./fixture";

gateNvimE2E();

nvimTest.describe("editor chrome and themes", () => {
  nvimTest("uses CSS custom-property tokens in glassy theme", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.page.evaluate(() => {
      localStorage.setItem("opman-theme-mode", "glassy");
      document.documentElement.classList.remove("flat-theme");
    });
    const values = await nvim.page.locator(".cm-nvim-status-panel").evaluate((element) => {
      const root = getComputedStyle(document.documentElement);
      const panel = getComputedStyle(element);
      return { token: root.getPropertyValue("--color-bg-panel").trim(), background: panel.backgroundColor };
    });
    expect(values.token).not.toBe("");
    expect(values.background).not.toBe("rgba(0, 0, 0, 0)");
  });

  nvimTest("uses the same tokenized chrome in flat theme", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.page.evaluate(() => {
      localStorage.setItem("opman-theme-mode", "flat");
      document.documentElement.classList.add("flat-theme");
    });
    await expect.poll(() => nvim.page.evaluate(() => document.documentElement.classList.contains("flat-theme"))).toBeTruthy();
    const values = await nvim.page.locator(".cm-nvim-status-panel").evaluate((element) => {
      const root = getComputedStyle(document.documentElement);
      const panel = getComputedStyle(element);
      return { token: root.getPropertyValue("--color-bg-panel").trim(), background: panel.backgroundColor };
    });
    expect(values.token).not.toBe("");
    expect(values.background).not.toBe("rgba(0, 0, 0, 0)");
  });
});
