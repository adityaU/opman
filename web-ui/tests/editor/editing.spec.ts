import { editorTest, expect, gateEditorE2E, mainFixtureText } from "./fixture";

gateEditorE2E();

editorTest.describe("CodeMirror editing", () => {
  editorTest("inserts typed text at the cursor", async ({ editor }) => {
    await editor.expectBufferText(mainFixtureText);
    await editor.setCursor({ line: 0, column: 0 });
    await editor.typeText("zeta ");
    await editor.expectBufferText(mainFixtureText.replace("alpha", "zeta alpha"));
    expect(await editor.cursorPosition()).toEqual({ line: 0, column: 5 });
  });

  editorTest("splits and joins lines with Enter and Backspace", async ({ editor }) => {
    await editor.setCursor({ line: 0, column: 19 });
    await editor.type("Enter");
    await editor.expectBufferText(mainFixtureText.replace("charlie\n", "charlie\n\n"));
    expect(await editor.cursorPosition()).toEqual({ line: 1, column: 0 });

    await editor.type("Backspace");
    await editor.expectBufferText(mainFixtureText);
    expect(await editor.cursorPosition()).toEqual({ line: 0, column: 19 });
  });

  editorTest("undoes and redoes an edit through the history keymap", async ({ editor }) => {
    await editor.setCursor({ line: 5, column: 0 });
    await editor.typeText("very ");
    const edited = mainFixtureText.replace("last line", "very last line");
    await editor.expectBufferText(edited);

    // `history` and `historyKeymap` used to be switched off whenever the
    // Neovim binding attached; they are unconditional now.
    await editor.type("ControlOrMeta+z");
    await editor.expectBufferText(mainFixtureText);
    await editor.type("ControlOrMeta+Shift+z");
    await editor.expectBufferText(edited);
  });

  editorTest("replaces the whole document through select-all", async ({ editor }) => {
    await editor.selectAll();
    await editor.typeText("only");
    await editor.expectBufferText("only");
  });

  editorTest("closes brackets and quotes as they are typed", async ({ editor }) => {
    await editor.openFile("definition.ts");
    // The trailing newline leaves an empty last line to type on.
    await editor.setCursor({ line: 3, column: 0 });
    await editor.typeText("const pair = (");
    await expect.poll(async () => (await editor.bufferText()).split("\n")[3]).toBe("const pair = ()");

    await editor.typeText("[");
    await expect.poll(async () => (await editor.bufferText()).split("\n")[3]).toBe("const pair = ([])");
  });

  editorTest("indents with Tab inside the document", async ({ editor }) => {
    await editor.setCursor({ line: 1, column: 0 });
    await editor.type("Tab");
    // `indentWithTab` was one of the keymaps the Neovim gate removed.
    await expect.poll(async () => (await editor.bufferText()).split("\n")[1])
      .toMatch(/^\s+one two three four five$/);
  });

  editorTest("edits a CRLF file without corrupting its line endings", async ({ editor }) => {
    await editor.openFile("crlf.txt");
    await editor.expectBufferText("first\nsecond\nthird\n");
    await editor.setCursor({ line: 1, column: 6 });
    await editor.typeText(" line");
    await editor.expectBufferText("first\nsecond line\nthird\n");
  });

  editorTest("keeps multibyte columns aligned while editing", async ({ editor }) => {
    await editor.openFile("unicode.txt");
    await editor.expectBufferText("hello 😀 世界\nemoji column risk\n");
    // The emoji is a surrogate pair, so column 8 is the character after it.
    await editor.setCursor({ line: 0, column: 9 });
    await editor.typeText("!");
    await editor.expectBufferText("hello 😀 !世界\nemoji column risk\n");
  });

  editorTest("scrolls and edits far down a five-thousand line file", async ({ editor }) => {
    await editor.openFile("large.txt");
    await expect.poll(async () => (await editor.bufferText()).split("\n").length).toBe(5501);
    await editor.setCursor({ line: 5000, column: 0 });
    await editor.typeText("x");
    await expect.poll(async () => (await editor.bufferText()).split("\n")[5000]).toBe("xline 5001");
  });
});
