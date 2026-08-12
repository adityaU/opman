import { expect, gateNvimE2E, nvimTest } from "./fixture";

gateNvimE2E();

nvimTest.describe("command line and messages", () => {
  nvimTest("renders opman's HTML command line for colon input", async ({ nvim }) => {
    await nvim.typeKeys(":");
    await expect(nvim.page.getByRole("region", { name: "Neovim command line" })).toBeVisible();
    await nvim.typeKeys("Escape");
  });

  nvimTest("supports command-line history and Escape cancellation", async ({ nvim }) => {
    await nvim.typeKeys(":", "echo \"hi\"", "Enter", ":", "ArrowUp");
    await expect(nvim.page.locator(".nvim-cmdline-content")).toContainText("echo");
    await nvim.typeKeys("Escape");
    await expect(nvim.page.getByRole("region", { name: "Neovim command line" })).toHaveCount(0);
  });

  nvimTest("renders Neovim messages, errors, history, and showmode", async ({ nvim }) => {
    await nvim.typeKeys(":", "echo \"hi\"", "Enter");
    await nvim.typeKeys(":", "echoerr \"bad\"", "Enter");
    await nvim.typeKeys(":", "messages", "Enter");
    await expect(nvim.page.locator(".nvim-messages-overlay")).toContainText("hi");
    await expect(nvim.page.locator(".nvim-message-error")).toContainText("bad");
    await nvim.typeKeys("i");
    await expect(nvim.page.locator(".nvim-statusline-showmode")).toContainText("INSERT");
    await nvim.typeKeys("Escape");
  });
});
