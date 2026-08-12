import { expect, gateNvimE2E, nvimTest } from "./fixture";

gateNvimE2E();

nvimTest.describe("Neovim modes", () => {
  nvimTest("reports normal and insert modes", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.expectMode("normal");
    await nvim.typeKeys("i");
    await nvim.expectMode("insert");
    await nvim.typeText("typed");
    await nvim.typeKeys("Escape");
    await nvim.expectMode("normal");
  });

  nvimTest("reports visual character, line and block selections", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("v", "l");
    await nvim.expectMode("visual");
    await expect(nvim.page.locator(".cm-nvim-visual-selection")).toHaveCount(1);
    await nvim.typeKeys("Escape", "Shift+v");
    await nvim.expectMode("visual_line");
    await nvim.typeKeys("Escape", "Control+v");
    await nvim.expectMode("visual_block");
  });

  nvimTest("reports operator-pending mode", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("d");
    await nvim.expectMode("operator_pending");
    await nvim.typeKeys("Escape");
    await nvim.expectMode("normal");
  });

  nvimTest("reports replace mode and leaves it on Escape", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("Shift+r");
    await nvim.expectMode("replace");
    await nvim.typeKeys("Escape");
    await nvim.expectMode("normal");
  });

  nvimTest("renders a Neovim connection status rather than a silent editor", async ({ nvim }) => {
    await nvim.seedMain();
    const status = nvim.page.locator(".cm-nvim-status-panel");
    await expect(status).toContainText("Neovim");
    await expect(status.locator(".cm-nvim-mode-label")).toHaveAttribute("data-mode", "normal");
  });
});
