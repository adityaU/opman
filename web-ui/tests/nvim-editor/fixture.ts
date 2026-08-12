/// <reference types="node" />

import { test as base, expect, type APIRequestContext, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { readFile, unlink, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";

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

interface RawEdit {
  readonly type: "edit";
  readonly changedtick: number;
  readonly start: { readonly line: number; readonly column: number };
  readonly end: { readonly line: number; readonly column: number };
  readonly lines: readonly string[];
  readonly edit_id: string;
}

interface ProcessEntry {
  readonly pid: number;
  readonly ppid: number;
  readonly command: string;
}

const mainFixtureText = [
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
};

export interface NvimFixture {
  readonly page: Page;
  readonly workspace: string;
  readonly backend: string;
  readonly typeKeys: (...keys: string[]) => Promise<void>;
  readonly typeText: (text: string) => Promise<void>;
  readonly expectMode: (mode: string) => Promise<void>;
  readonly expectBufferText: (text: string) => Promise<void>;
  readonly bufferText: () => Promise<string>;
  readonly cursorPosition: () => Promise<{ line: number; column: number } | null>;
  readonly openFile: (path: string) => Promise<void>;
  readonly readFileOnDisk: (path: string) => Promise<string>;
  readonly writeFileOnDisk: (path: string, text: string) => Promise<void>;
  readonly seedBuffer: (path: string, text: string) => Promise<void>;
  readonly seedMain: () => Promise<void>;
  readonly killNvim: () => Promise<void>;
  readonly sendRawEdit: (message: RawEdit) => Promise<void>;
}

interface Fixtures {
  readonly nvim: NvimFixture;
}

export const nvimTest = base.extend<Fixtures>({
  nvim: async ({ page }, use, testInfo) => {
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
    const keybindings = await request.put("/api/keybindings", {
      data: {
        mode: "vim",
        leader: "<space>",
        localLeader: ",",
        chordTimeoutMs: 500,
        whichKey: { enabled: true, delayMs: 50, sortBy: "group" },
        bindings: [],
      },
      headers: { Authorization: `Bearer ${token}` },
    });
    expect(keybindings.ok()).toBeTruthy();
    await request.dispose();
    await page.context().addCookies([{
      name: "opman_token",
      value: token as string,
      domain: "localhost",
      path: "/",
      httpOnly: true,
    }]);
    await page.addInitScript(
      ({ workspace, sessionId }: { readonly workspace: string; readonly sessionId: string }) => {
        localStorage.setItem("opman-theme-mode", "glassy");
        localStorage.setItem("opman.editor-engine", JSON.stringify({ version: 1, engine: "neovim" }));
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
            chrome: { rail: true, paneHeaders: true, zen: false },
          },
        }));
      },
      { workspace: state.workspace, sessionId: testInfo.testId },
    );
    await page.addInitScript(() => {
      const windowWithSocket = window as Window & { __opmanNvimSocket?: WebSocket };
      const NativeWebSocket = window.WebSocket;
      window.WebSocket = new Proxy(NativeWebSocket, {
        construct(target, args, newTarget) {
          const socket = Reflect.construct(target, args, newTarget) as WebSocket;
          if (String(args[0]).includes("/api/nvim/ui")) windowWithSocket.__opmanNvimSocket = socket;
          return socket;
        },
      });
    });
    await page.goto("http://localhost:5199", { waitUntil: "domcontentloaded" });
    await page.locator(".code-editor-panel").waitFor();
    await expect(page.locator(".code-editor-filename")).toHaveText("main.txt");
    await page.locator(".cm-content").waitFor();
    await expect(page.locator(".cm-nvim-connection-label")).toHaveText("Neovim", { timeout: 15_000 });

    const editor = page.locator(".cm-content");
    // Neither click nor focus is free: a click moves CodeMirror's caret and
    // cancels a pending Neovim operator, and focusing an already-focused
    // contenteditable resets its selection. Only reach for focus when the
    // editor does not already have it.
    const ensureFocus = async (): Promise<void> => {
      if (await editor.evaluate((node) => node === document.activeElement)) return;
      await editor.focus();
    };
    const typeKeys = async (...keys: string[]): Promise<void> => {
      await ensureFocus();
      const named = new Set(["Escape", "Enter", "Space", "Tab", "Backspace", "Delete", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "F2", "F8", "F12"]);
      for (const key of keys) {
        if (named.has(key) || key.includes("+")) {
          await editor.press(key);
        } else {
          for (const character of [...key]) await editor.press(character);
        }
      }
    };
    // CodeMirror only renders the lines near the viewport, so reading `.cm-line`
    // would silently truncate any file taller than the screen. Read the
    // document itself and keep the DOM join as a fallback.
    const bufferText = async (): Promise<string> =>
      page.evaluate(() => {
        interface Tile { readonly root?: { readonly view?: { readonly state: { readonly doc: { toString(): string } } } } }
        const content = document.querySelector(".cm-content") as (Element & { cmTile?: Tile }) | null;
        const doc = content?.cmTile?.root?.view?.state.doc;
        if (doc) return doc.toString();
        return [...document.querySelectorAll(".cm-line")].map((line) => line.textContent ?? "").join("\n");
      });
    const typeText = async (text: string): Promise<void> => {
      await ensureFocus();
      let previous = await bufferText();
      for (const character of [...text]) {
        await editor.press(character);
        await expect.poll(bufferText).not.toBe(previous);
        previous = await bufferText();
      }
    };
    // Neovim reads a CRLF file into LF buffer lines and writes the CRLF back
    // out, so a buffer comparison is always against normalized text.
    const expectBufferText = async (text: string): Promise<void> => {
      const normalized = text.replaceAll("\r\n", "\n");
      const expected = normalized.endsWith("\n") ? normalized.slice(0, -1) : normalized;
      await expect.poll(bufferText, { timeout: 5_000 }).toBe(expected);
    };
    const expectMode = async (mode: string): Promise<void> => {
      await expect(page.locator(".cm-nvim-mode-label")).toHaveAttribute("data-mode", mode);
    };
    const cursorPosition = async (): Promise<{ line: number; column: number } | null> =>
      page.locator(".cm-content").evaluate((content) => {
        const marker = content.querySelector<HTMLElement>(".cm-nvim-cursor");
        if (!marker) return null;
        const line = marker.closest<HTMLElement>(".cm-line");
        if (!line) return null;
        const lines = [...content.querySelectorAll(".cm-line")];
        const range = document.createRange();
        range.selectNodeContents(line);
        range.setEndBefore(marker);
        return { line: lines.indexOf(line), column: range.toString().length };
      });
    const openFile = async (path: string): Promise<void> => {
      const row = page.locator(`button.xpl-entry-file[title="${path}"]`);
      await row.waitFor();
      await row.click();
      await expect(page.locator(".code-editor-filename")).toHaveText(path);
      await page.locator(".cm-content").waitFor();
    };
    const readFileOnDisk = async (path: string): Promise<string> => readFile(join(state.workspace, path), "utf8");
    const writeFileOnDisk = async (path: string, text: string): Promise<void> => writeFile(join(state.workspace, path), text);
    const seedBuffer = async (path: string, text: string): Promise<void> => {
      await writeFileOnDisk(path, text);
      if (await page.locator(".code-editor-filename").textContent() !== path) await openFile(path);
      await typeKeys(":", "e!", "Enter");
      await expect(page.locator(".code-editor-filename")).toHaveText(path);
      await expectBufferText(text);
      await typeKeys("Escape", "g", "g", "0");
      await expectMode("normal");
      await expect.poll(cursorPosition).toMatchObject({ line: 0, column: 0 });
    };
    const seedMain = async (): Promise<void> => seedBuffer("main.txt", mainFixtureText);
    const killNvim = async (): Promise<void> => {
      const nvim = descendantsOf(processEntries(), state.opmanPid)
        .filter((entry) => /^nvim(?:\s|$)/.test(entry.command))
        .sort((left, right) => right.pid - left.pid)[0];
      if (!nvim) throw new Error(`No spawned Neovim child found below opman ${state.opmanPid}`);
      process.kill(nvim.pid, "SIGTERM");
    };
    const sendRawEdit = async (message: RawEdit): Promise<void> => {
      await page.evaluate((payload: RawEdit) => {
        const windowWithSocket = window as Window & { __opmanNvimSocket?: WebSocket };
        const socket = windowWithSocket.__opmanNvimSocket;
        if (!socket || socket.readyState !== WebSocket.OPEN) throw new Error("Neovim WebSocket is not open");
        socket.send(JSON.stringify(payload));
      }, message);
    };

    await use({ page, workspace: state.workspace, backend: state.backend, typeKeys, typeText, expectMode, expectBufferText, bufferText, cursorPosition, openFile, readFileOnDisk, writeFileOnDisk, seedBuffer, seedMain, killNvim, sendRawEdit });
    await page.close();
    await unlink(join("/tmp", `opman-agent-manager-${state.opmanPid}.sock`)).catch(() => {});
  },
});

export function gateNvimE2E(): void {
  nvimTest.skip(process.env.OPMAN_E2E_NVIM !== "1", "Set OPMAN_E2E_NVIM=1 to run the hermetic Neovim suite");
}

async function baseRequest(backend: string): Promise<APIRequestContext> {
  const { request } = await import("@playwright/test");
  return request.newContext({ baseURL: backend });
}

export { expect };

function processEntries(): readonly ProcessEntry[] {
  return execFileSync("ps", ["-eo", "pid=,ppid=,args="], { encoding: "utf8" })
    .split("\n")
    .map((line) => /^\s*(\d+)\s+(\d+)\s+(.*)$/.exec(line))
    .filter((match): match is RegExpExecArray => match !== null)
    .map((match) => ({ pid: Number(match[1]), ppid: Number(match[2]), command: match[3] }));
}

function descendantsOf(entries: readonly ProcessEntry[], root: number): readonly ProcessEntry[] {
  const result: ProcessEntry[] = [];
  const pending = [root];
  while (pending.length > 0) {
    const parent = pending.shift();
    if (parent === undefined) continue;
    for (const entry of entries) {
      if (entry.ppid !== parent || result.some((item) => item.pid === entry.pid)) continue;
      result.push(entry);
      pending.push(entry.pid);
    }
  }
  return result;
}
