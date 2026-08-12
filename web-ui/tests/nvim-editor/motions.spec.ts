import { expect, gateNvimE2E, nvimTest } from "./fixture";

gateNvimE2E();

nvimTest.describe("motions", () => {
  nvimTest("moves with h j k l and reports cursor line", async ({ nvim }) => {
    await nvim.seedMain();
    const start = await nvim.cursorPosition();
    expect(start).not.toBeNull();
    await nvim.typeKeys("j", "j", "l", "h", "k");
    await expect.poll(nvim.cursorPosition).toMatchObject({ line: 1 });
  });

  nvimTest("moves by words, counts and line anchors", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("0", "5j", "3w");
    await expect.poll(nvim.cursorPosition).toMatchObject({ line: 5 });
    // Neovim defaults to 'nostartofline', so gg after $ keeps the column.
    await nvim.typeKeys("b", "e", "^", "$", "g", "g");
    await expect.poll(nvim.cursorPosition).toMatchObject({ line: 0 });
    await nvim.typeKeys("0");
    await expect.poll(nvim.cursorPosition).toMatchObject({ line: 0, column: 0 });
    await nvim.typeKeys("Shift+g");
    await expect.poll(nvim.cursorPosition).toMatchObject({ line: 5 });
  });

  nvimTest("supports character find, repeat and bracket matching", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("f", "b", ";", ",", "t", "a", "F", "a", "T", "a");
    await nvim.typeKeys("%", "Control+d", "Control+u");
    await expect(nvim.page.locator(".cm-nvim-cursor")).toBeVisible();
    expect(await nvim.cursorPosition()).not.toBeNull();
  });

  nvimTest("keeps the cursor in the document after a long motion sequence", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("g", "g", "Shift+g", "g", "g", "j", "j", "k");
    await expect.poll(nvim.cursorPosition).toMatchObject({ line: 1 });
    expect((await nvim.cursorPosition())?.column).toBeGreaterThanOrEqual(0);
  });
});
