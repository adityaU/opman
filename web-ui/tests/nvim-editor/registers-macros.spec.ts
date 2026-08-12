import { expect, gateNvimE2E, nvimTest } from "./fixture";

gateNvimE2E();

nvimTest.describe("registers and macros", () => {
  nvimTest("yanks to a named register and pastes it", async ({ nvim }) => {
    await nvim.seedMain();
    // Register a holds the first line; pasting after the last line duplicates it.
    await nvim.typeKeys('"', "a", "y", "y", "G", '"', "a", "p");
    await expect.poll(async () =>
      (await nvim.bufferText()).split("\n").filter((line) => line === "alpha bravo charlie").length,
    ).toBe(2);
  });

  nvimTest("records and replays a macro once and by count", async ({ nvim }) => {
    await nvim.seedMain();
    await nvim.typeKeys("q", "a", "0", "x", "q", "@", "a", "3@a");
    await expect.poll(nvim.bufferText).not.toContain("alpha bravo charlie");
  });
});
