import { editorTest, expect, gateEditorE2E, mainFixtureText } from "./fixture";

gateEditorE2E();

editorTest.describe("file editing lifecycle", () => {
  editorTest("opens a file from the explorer and shows its contents", async ({ editor }) => {
    await editor.openFile("second.txt");
    await editor.expectBufferText("second buffer\nkeep this line\n");
    await expect(editor.page.locator(".code-editor-filename")).toHaveText("second.txt");
  });

  editorTest("keeps each file's own text when switching between them", async ({ editor }) => {
    await editor.setCursor({ line: 0, column: 0 });
    await editor.typeText("edited ");
    await editor.openFile("second.txt");
    await editor.expectBufferText("second buffer\nkeep this line\n");

    await editor.openFile("main.txt");
    await editor.expectBufferText(mainFixtureText.replace("alpha", "edited alpha"));
  });

  editorTest("marks the file dirty on edit and writes it to disk on save", async ({ editor }) => {
    await editor.setCursor({ line: 5, column: 0 });
    await editor.typeText("saved ");
    const expected = mainFixtureText.replace("last line", "saved last line");
    await editor.expectBufferText(expected);

    await editor.save();
    await expect.poll(() => editor.readFileOnDisk("main.txt"), { timeout: 10_000 }).toBe(expected);
  });

  editorTest("reflects a file rewritten on disk when it is reopened", async ({ editor }) => {
    await editor.writeFileOnDisk("second.txt", "rewritten on disk\n");
    await editor.openFile("second.txt");
    await editor.expectBufferText("rewritten on disk\n");
  });

  editorTest("reports the language for a TypeScript file", async ({ editor }) => {
    await editor.openFile("error.ts");
    await editor.expectBufferText(
      "const answer: string = 42;\nfunction useAnswer() { return answer; }\nexport {};\n",
    );
    // A plain-text render produces bare text nodes; the language mode is what
    // wraps tokens in styled spans.
    await expect.poll(() => editor.page.locator(".cm-line span").count()).toBeGreaterThan(0);
  });
});
