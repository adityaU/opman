import { mkdirSync, mkdtempSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { execFileSync, spawn } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..", "..");
const statePath = join(here, ".state.json");
const tempRoot = mkdtempSync(join("/tmp", "opman-editor-e2e-"));
const workspace = join(tempRoot, "workspace");
const configHome = join(tempRoot, "config");
const dataHome = join(tempRoot, "data");
const stateHome = join(tempRoot, "state");
const cacheHome = join(tempRoot, "cache");
mkdirSync(workspace, { recursive: true });
mkdirSync(join(configHome, "opman"), { recursive: true });
mkdirSync(dataHome, { recursive: true });
mkdirSync(stateHome, { recursive: true });
mkdirSync(cacheHome, { recursive: true });

const mainText = [
  "alpha bravo charlie",
  "one two three four five",
  "function greet(name) { return `Hello ${name}`; }",
  "brackets (square [curly {nested}])",
  "needle needle needle",
  "last line",
].join("\n") + "\n";

writeFileSync(join(workspace, "main.txt"), mainText);
writeFileSync(join(workspace, "second.txt"), "second buffer\nkeep this line\n");
writeFileSync(join(workspace, "crlf.txt"), "first\r\nsecond\r\nthird\r\n");
writeFileSync(join(workspace, "unicode.txt"), "hello 😀 世界\nemoji column risk\n");
writeFileSync(join(workspace, "error.ts"), "const answer: string = 42;\nfunction useAnswer() { return answer; }\nexport {};\n");
writeFileSync(join(workspace, "definition.ts"), "const answer = 42;\nconsole.log(answer);\nexport {};\n");
writeFileSync(join(workspace, "large.txt"), Array.from({ length: 5500 }, (_, index) => `line ${index + 1}`).join("\n") + "\n");
writeFileSync(join(workspace, "fold.txt"), "function outer() {\n  const value = 42;\n  return value;\n}\n");
writeFileSync(join(workspace, "objects.txt"), "quotes \"inside\" here\nwrapped (inside) here\n");
// A cross-file jump target: `shared` is declared in lib.ts and used in consumer.ts.
writeFileSync(join(workspace, "lib.ts"), 'export const shared = 1;\nexport type Shape = { size: number };\n');
writeFileSync(join(workspace, "consumer.ts"), 'import { shared, type Shape } from "./lib";\nconst box: Shape = { size: shared };\nconsole.log(box);\n');
// A project root, so the TypeScript language server treats the fixture files
// as one program rather than refusing to start.
writeFileSync(join(workspace, "tsconfig.json"), JSON.stringify({
  compilerOptions: { target: "ES2020", module: "ESNext", moduleResolution: "bundler", strict: true, noEmit: true },
  include: ["*.ts"],
}, null, 2) + "\n");
writeFileSync(join(configHome, "opman", "config.toml"), `[[projects]]\nname = "editor-e2e"\npath = ${JSON.stringify(workspace)}\n`);
execFileSync("git", ["init", "-q"], { cwd: workspace });
execFileSync("git", ["config", "user.email", "e2e@example.invalid"], { cwd: workspace });
execFileSync("git", ["config", "user.name", "opman e2e"], { cwd: workspace });

// The hermetic backend must find a language server without depending on what
// happens to be installed globally on the machine running the suite.
const localBin = join(repo, "web-ui", "node_modules", ".bin");

const env = {
  ...process.env,
  PATH: `${localBin}:${process.env.PATH ?? ""}`,
  XDG_CONFIG_HOME: configHome,
  XDG_DATA_HOME: dataHome,
  XDG_STATE_HOME: stateHome,
  XDG_CACHE_HOME: cacheHome,
};
const binary = process.env.OPMAN_E2E_BINARY ?? join(repo, "target", "release", "opman");
const username = "e2e";
const password = "e2e-password";
const opman = spawn(binary, [
  "--web",
  "--web-only",
  "--web-port",
  "0",
  "--web-user",
  username,
  "--web-pass",
  password,
], { cwd: workspace, env, stdio: ["ignore", "pipe", "pipe"] });

let output = "";
opman.stdout.setEncoding("utf8");
opman.stderr.setEncoding("utf8");
opman.stdout.on("data", (chunk) => { output += chunk; });
opman.stderr.on("data", (chunk) => { output += chunk; if (process.env.OPMAN_E2E_ECHO) process.stderr.write(chunk); });

function portsFrom(text) {
  return [...text.matchAll(/(?:localhost|127\.0\.0\.1|0\.0\.0\.0):(\d+)/g)].map((match) => Number(match[1]));
}

async function probe(port) {
  try {
    const response = await fetch(`http://127.0.0.1:${port}/api/state`);
    return response.status === 401 || response.status === 200;
  } catch {
    return false;
  }
}

async function findBackend() {
  const deadline = Date.now() + 45_000;
  while (Date.now() < deadline) {
    for (const port of [...new Set(portsFrom(output))]) {
      if (await probe(port)) return `http://127.0.0.1:${port}`;
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
  }
  throw new Error(`Could not find opman API listener. Output:\n${output}`);
}

async function stop(child, signal) {
  if (child.exitCode !== null) return Promise.resolve();
  child.kill(signal);
  await Promise.race([
    new Promise((resolvePromise) => child.once("exit", resolvePromise)),
    new Promise((resolvePromise) => setTimeout(resolvePromise, 5_000)),
  ]);
  if (child.exitCode === null) {
    child.kill("SIGKILL");
    await new Promise((resolvePromise) => {
      if (child.exitCode !== null) resolvePromise();
      else child.once("exit", resolvePromise);
    });
  }
}

function descendantsOf(rootPid) {
  const rows = execFileSync("ps", ["-eo", "pid=,ppid="], { encoding: "utf8" })
    .split("\n")
    .map((line) => /^\s*(\d+)\s+(\d+)$/.exec(line))
    .filter((match) => match !== null)
    .map((match) => ({ pid: Number(match[1]), ppid: Number(match[2]) }));
  const descendants = [];
  const pending = [rootPid];
  while (pending.length > 0) {
    const parent = pending.shift();
    if (parent === undefined) continue;
    for (const row of rows) {
      if (row.ppid !== parent || descendants.includes(row.pid)) continue;
      descendants.push(row.pid);
      pending.push(row.pid);
    }
  }
  return descendants;
}

function killPids(pids) {
  for (const pid of pids.reverse()) {
    try { process.kill(pid, "SIGTERM"); } catch {}
  }
}

let vite;
let cleanupStarted = false;
const managerSocket = join("/tmp", `opman-agent-manager-${opman.pid}.sock`);

async function cleanup() {
  if (cleanupStarted) return;
  cleanupStarted = true;
  if (vite && vite.exitCode === null) await stop(vite, "SIGTERM");
  const opmanChildren = descendantsOf(opman.pid);
  await stop(opman, "SIGINT").catch(() => {});
  if (opman.exitCode === null) await stop(opman, "SIGTERM").catch(() => {});
  killPids(opmanChildren);
  try { unlinkSync(managerSocket); } catch {}
  try { unlinkSync(statePath); } catch {}
  rmSync(tempRoot, { recursive: true, force: true });
}

process.on("SIGTERM", () => { void cleanup().then(() => process.exit(process.exitCode ?? 0)); });
process.on("SIGINT", () => { void cleanup().then(() => process.exit(process.exitCode ?? 0)); });
process.on("exit", () => { try { unlinkSync(managerSocket); } catch {} });

try {
  const backend = await findBackend();
  writeFileSync(statePath, JSON.stringify({ backend, workspace, username, password, opmanPid: opman.pid }));
  vite = spawn(process.execPath, [join(repo, "web-ui/node_modules/vite/bin/vite.js"), "--port", "5199", "--strictPort"], {
    cwd: join(repo, "web-ui"),
    env: { ...process.env, OPMAN_E2E_BACKEND: backend },
    stdio: "inherit",
  });
  const exitCode = await new Promise((resolvePromise) => vite.once("exit", (code) => resolvePromise(code ?? 1)));
  if (exitCode !== 0) process.exitCode = exitCode;
} catch (error) {
  console.error(error);
  process.exitCode = 1;
} finally {
  await cleanup();
}
