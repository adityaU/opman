import { expect, gateNvimE2E, nvimTest } from "./fixture";

nvimTest.describe("debug", () => {
  gateNvimE2E();
  nvimTest("debug insert", async ({ nvim }) => {
    nvim.page.on("console", (message) => console.log(`PAGE ${message.text()}`));
    await nvim.seedMain();
    console.log("--- start ---");
    await nvim.typeKeys("i");
    await nvim.page.waitForTimeout(2500);
    console.log(`MODE ${await nvim.page.locator(".cm-nvim-mode-label").getAttribute("data-mode")}`);
    expect(true).toBe(true);
  });
});
