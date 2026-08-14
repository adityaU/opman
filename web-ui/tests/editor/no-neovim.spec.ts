import { editorTest, expect, gateEditorE2E, mainFixtureText } from "./fixture";

gateEditorE2E();

editorTest.describe("the editor has no Neovim mode", () => {
  editorTest("renders CodeMirror with no grid canvas, overlay or status panel", async ({ editor }) => {
    await editor.expectBufferText(mainFixtureText);
    await expect(editor.page.locator(".cm-content")).toBeVisible();

    const panel = editor.page.locator(".code-editor-panel");
    await expect(panel.locator(".nvg-surface, .nvg-canvas, .nvg-window")).toHaveCount(0);
    await expect(panel.locator(".cm-nvim-status-panel, .cm-nvim-overlay-panel")).toHaveCount(0);
    await expect(panel.locator(".nvim-cmdline-overlay, .nvim-popupmenu-overlay, .nvim-tabline-overlay"))
      .toHaveCount(0);
    await expect(panel.locator("[data-nvim-capture]")).toHaveCount(0);
    // The editor must not be a terminal running Neovim either.
    await expect(panel.locator(".xterm, .xterm-screen")).toHaveCount(0);
  });

  editorTest("opens no Neovim websocket and calls no Neovim endpoint", async ({ editor, page }) => {
    const sockets: string[] = [];
    const requests: string[] = [];
    page.on("websocket", (socket) => sockets.push(socket.url()));
    page.on("request", (request) => requests.push(request.url()));

    await editor.openFile("second.txt");
    await editor.setCursor({ line: 0, column: 0 });
    await editor.typeText("x");
    await editor.expectBufferText("xsecond buffer\nkeep this line\n");

    expect(sockets.filter((url) => url.includes("/api/nvim"))).toEqual([]);
    expect(requests.filter((url) => url.includes("/api/nvim"))).toEqual([]);
  });

  editorTest("types plain text where a Vim normal-mode key would have been a command", async ({ editor }) => {
    await editor.setCursor({ line: 0, column: 0 });
    // In Neovim mode `dd` deleted the line and `i` entered insert without
    // inserting. Every one of these is now literal input.
    await editor.typeText("ddi");
    await editor.expectBufferText(mainFixtureText.replace("alpha", "ddialpha"));
  });
});
