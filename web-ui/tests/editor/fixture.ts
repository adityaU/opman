/// <reference types="node" />

import { test as base, expect, type APIRequestContext, type Page } from "@playwright/test";
import { readFile, unlink, writeFile } from "node:fs/promises";
import { join } from "node:path";

interface StateFile {
  readonly backend: string;
  readonly workspace: string;
  readonly username: string;
  readonly password: string;
  readonly opmanPid: number;
}

interface LoginBody {
  readonly token?: unknown;
}

export interface CursorPosition {
  readonly line: number;
  readonly column: number;
}

export const mainFixtureText = [
  "alpha bravo charlie",
  "one two three four five",
  "function greet(name) { return `Hello ${name}`; }",
  "brackets (square [curly {nested}])",
  "needle needle needle",
  "last line",
].join("\n") + "\n";

const fixtureFiles: Readonly<Record<string, string>> = {
  "main.txt": mainFixtureText,
  "second.txt": "second buffer\nkeep this line\n",
  "crlf.txt": "first\r\nsecond\r\nthird\r\n",
  "unicode.txt": "hello 😀 世界\nemoji column risk\n",
  "error.ts": "const answer: string = 42;\nfunction useAnswer() { return answer; }\nexport {};\n",
  "definition.ts": "const answer = 42;\nconsole.log(answer);\nexport {};\n",
  "large.txt": Array.from({ length: 5500 }, (_, index) => `line ${index + 1}`).join("\n") + "\n",
  "fold.txt": "function outer() {\n  const value = 42;\n  return value;\n}\n",
  "objects.txt": "quotes \"inside\" here\nwrapped (inside) here\n",
  "lib.ts": 'export const shared = 1;\nexport type Shape = { size: number };\n',
  "consumer.ts": 'import { shared, type Shape } from "./lib";\nconst box: Shape = { size: shared };\nconsole.log(box);\n',
};

export interface EditorFixture {
  readonly page: Page;
  readonly workspace: string;
  readonly backend: string;
  /** Press keys in order. A multi-character string is typed character by character. */
  readonly type: (...keys: string[]) => Promise<void>;
  /** Type literal text, waiting for each character to land in the document. */
  readonly typeText: (text: string) => Promise<void>;
  readonly bufferText: () => Promise<string>;
  readonly expectBufferText: (text: string) => Promise<void>;
  readonly cursorPosition: () => Promise<CursorPosition>;
  readonly setCursor: (position: CursorPosition) => Promise<void>;
  readonly selectAll: () => Promise<void>;
  readonly openFile: (path: string) => Promise<void>;
  readonly save: () => Promise<void>;
  readonly readFileOnDisk: (path: string) => Promise<string>;
  readonly writeFileOnDisk: (path: string, text: string) => Promise<void>;
}

interface Fixtures {
  readonly editor: EditorFixture;
}

/** The CodeMirror view is reachable from the content element the panel mounts. */
interface ContentWithView extends Element {
  readonly cmTile?: {
    readonly root?: {
      readonly view?: {
        readonly state: {
          readonly doc: { toString(): string; line(n: number): { readonly from: number } };
          readonly selection: { readonly main: { readonly head: number } };
        };
        dispatch(spec: unknown): void;
        focus(): void;
      };
    };
  };
}

export const editorTest = base.extend<Fixtures>({
  editor: async ({ page }, use, testInfo) => {
    const state = JSON.parse(await readFile(new URL("./.state.json", import.meta.url), "utf8")) as StateFile;
    await Promise.all(Object.entries(fixtureFiles).map(([path, text]) => writeFile(join(state.workspace, path), text)));
    await unlink(join(state.workspace, "created.txt")).catch(() => {});

    const request = await baseRequest(state.backend);
    const login = await request.post("/api/auth/login", {
      data: { username: state.username, password: state.password },
    });
    expect(login.ok()).toBeTruthy();
    const body = (await login.json()) as LoginBody;
    const token = typeof body.token === "string" ? body.token : null;
    expect(token).not.toBeNull();
    await request.dispose();

    await page.context().addCookies([{
      name: "opman_token", value: token as string, domain: "localhost", path: "/", httpOnly: true,
    }]);
    await page.addInitScript(
      ({ workspace, sessionId }: { readonly workspace: string; readonly sessionId: string }) => {
        localStorage.setItem("opman-theme-mode", "glassy");
        localStorage.setItem("opman.workspace", JSON.stringify({
          version: 1,
          workspace: {
            windows: [{
              id: "e2e-window",
              name: "1",
              root: {
                type: "leaf",
                id: "e2e-pane",
                widget: {
                  kind: "files",
                  projectPath: workspace,
                  sessionId,
                  open: { path: "main.txt", line: null, seq: 1 },
                },
              },
              focusedPaneId: "e2e-pane",
              zoomedPaneId: null,
            }],
            activeWindowId: "e2e-window",
            chrome: { rail: true, zen: false },
          },
        }));
      },
      { workspace: state.workspace, sessionId: testInfo.testId },
    );

    await page.goto("http://localhost:5199", { waitUntil: "domcontentloaded" });
    await page.locator(".code-editor-panel").waitFor();
    await expect(page.locator(".code-editor-filename")).toHaveText("main.txt");
    await page.locator(".cm-content").waitFor();

    const content = page.locator(".cm-content");
    const ensureFocus = async (): Promise<void> => {
      if (await content.evaluate((node) => node === document.activeElement)) return;
      await content.focus();
    };
    // CodeMirror only renders lines near the viewport, so joining `.cm-line`
    // would silently truncate any file taller than the screen. Read the
    // document itself and keep the DOM join as a fallback.
    const bufferText = async (): Promise<string> =>
      page.evaluate(() => {
        const node = document.querySelector(".cm-content") as (Element & { cmTile?: unknown }) | null;
        const doc = (node as ContentWithView | null)?.cmTile?.root?.view?.state.doc;
        if (doc) return doc.toString();
        return [...document.querySelectorAll(".cm-line")].map((line) => line.textContent ?? "").join("\n");
      });
    const type = async (...keys: string[]): Promise<void> => {
      await ensureFocus();
      const named = new Set([
        "Escape", "Enter", "Space", "Tab", "Backspace", "Delete",
        "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End",
        "F2", "F8", "F12",
      ]);
      for (const key of keys) {
        if (named.has(key) || key.includes("+")) {
          await content.press(key);
          continue;
        }
        for (const character of [...key]) await content.press(character);
      }
    };
    // Pressing keys faster than CodeMirror commits them loses characters, so
    // each one is confirmed against the document before the next is sent.
    const typeText = async (text: string): Promise<void> => {
      await ensureFocus();
      let previous = await bufferText();
      for (const character of [...text]) {
        await content.press(character);
        await expect.poll(bufferText).not.toBe(previous);
        previous = await bufferText();
      }
    };
    const expectBufferText = async (text: string): Promise<void> => {
      const normalized = text.replaceAll("\r\n", "\n");
      await expect.poll(bufferText, { timeout: 5_000 }).toBe(normalized);
    };
    const cursorPosition = async (): Promise<CursorPosition> =>
      page.evaluate(() => {
        const node = document.querySelector(".cm-content") as ContentWithView | null;
        const view = node?.cmTile?.root?.view;
        if (!view) throw new Error("CodeMirror view is not mounted");
        const head = view.state.selection.main.head;
        const doc = view.state.doc as unknown as {
          lineAt(pos: number): { readonly number: number; readonly from: number };
        };
        const line = doc.lineAt(head);
        return { line: line.number - 1, column: head - line.from };
      });
    const setCursor = async (position: CursorPosition): Promise<void> => {
      await ensureFocus();
      await page.evaluate((target: CursorPosition) => {
        const node = document.querySelector(".cm-content") as ContentWithView | null;
        const view = node?.cmTile?.root?.view;
        if (!view) throw new Error("CodeMirror view is not mounted");
        const line = view.state.doc.line(target.line + 1);
        view.dispatch({ selection: { anchor: line.from + target.column } });
        view.focus();
      }, position);
      await expect.poll(cursorPosition).toEqual(position);
    };
    const selectAll = async (): Promise<void> => {
      await ensureFocus();
      await content.press("ControlOrMeta+a");
    };
    const openFile = async (path: string): Promise<void> => {
      const row = page.locator(`button.xpl-entry-file[title="${path}"]`);
      await row.waitFor();
      await row.click();
      await expect(page.locator(".code-editor-filename")).toHaveText(path);
      await page.locator(".cm-content").waitFor();
    };
    const readFileOnDisk = async (path: string): Promise<string> => readFile(join(state.workspace, path), "utf8");
    const writeFileOnDisk = async (path: string, text: string): Promise<void> =>
      writeFile(join(state.workspace, path), text);
    const save = async (): Promise<void> => {
      await ensureFocus();
      await content.press("ControlOrMeta+s");
    };

    await use({
      page, workspace: state.workspace, backend: state.backend,
      type, typeText, bufferText, expectBufferText, cursorPosition, setCursor, selectAll,
      openFile, save, readFileOnDisk, writeFileOnDisk,
    });
    await page.close();
    await unlink(join("/tmp", `opman-agent-manager-${state.opmanPid}.sock`)).catch(() => {});
  },
});

/**
 * Skip the whole file unless the hermetic harness was asked for.
 *
 * Registered as a hook rather than called at file scope: `test.skip(condition)`
 * outside a `describe` is only accepted by accident, and it throws outright
 * when a single spec file is run on its own.
 */
export function gateEditorE2E(): void {
  editorTest.beforeEach(() => {
    editorTest.skip(
      process.env.OPMAN_E2E_EDITOR !== "1",
      "Set OPMAN_E2E_EDITOR=1 to run the hermetic editor suite",
    );
  });
}

async function baseRequest(backend: string): Promise<APIRequestContext> {
  const { request } = await import("@playwright/test");
  return request.newContext({ baseURL: backend });
}

export { expect };
