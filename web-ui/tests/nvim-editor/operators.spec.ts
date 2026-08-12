import { expect, gateNvimE2E, nvimTest } from "./fixture";

gateNvimE2E();

nvimTest.describe("operators and text objects", () => {
  nvimTest("deletes, changes, yanks and pastes observable text", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("d", "d");
    await expect.poll(nvim.bufferText).not.toContain("alpha bravo charlie");
    await nvim.typeKeys("u", "Control+r");
    await expect.poll(nvim.bufferText).toContain("alpha bravo charlie");
    await nvim.typeKeys("0", "y", "y", "p", "P", "x", "s");
    await nvim.expectMode("insert");
  });

  nvimTest("handles word and line operators", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("0", "d", "w");
    await expect.poll(nvim.bufferText).toContain("bravo");
    await nvim.typeKeys("c", "w");
    await nvim.expectMode("insert");
    await nvim.typeText("replacement");
    await nvim.typeKeys("Escape", "Shift+d", "c", "c");
    await nvim.expectMode("insert");
  });

  nvimTest("supports inner and around text objects", async ({ nvim }) => {
    await nvim.seedBuffer("main.txt", "quotes \"inside\" here\nwrapped (inside) here\n");
    await nvim.typeKeys("0", "f", '"', "l", "d", "i", '"');
    await nvim.expectBufferText("quotes \"\" here\nwrapped (inside) here\n");
    await nvim.typeKeys("u");
    await nvim.typeKeys("0", "f", '"', "l", "d", "a", '"');
    await nvim.expectBufferText("quotes here\nwrapped (inside) here\n");
    await nvim.seedBuffer("main.txt", "quotes \"inside\" here\nwrapped (inside) here\n");
    await nvim.typeKeys("g", "j", "0", "f", "(", "l", "c", "i", "(");
    await nvim.expectMode("insert");
    await nvim.typeText("replacement");
    await nvim.typeKeys("Escape");
    await nvim.expectBufferText("quotes \"inside\" here\nwrapped (replacement) here\n");
  });

  nvimTest("dot repeats the last edit", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("0", "x", ".");
    await expect.poll(nvim.bufferText).not.toContain("alpha bravo charlie");
  });
});
