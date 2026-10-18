import type { MessagePart } from "../types";

/**
 * opman's own coding tools, shown the way the agents' built-in ones are.
 *
 * The `opman-tools` MCP server answers with structured JSON, which is right for code mode
 * and wrong for a person reading a transcript. Its tools are the same eight jobs as the
 * built-ins, so here they take the built-ins' names (a run of them reads "3 Grep calls", not
 * "3 Mcp opman tools grep calls") and their results are turned back into the plain text the
 * existing cards already know how to draw.
 */

const PREFIX = /^(?:mcp__)?opman[-_]tools(?:__|_)(.+)$/i;

/** Leaf name -> the name the built-in tool of the same job goes by. */
const NATIVE: Readonly<Record<string, string>> = {
  bash: "Bash",
  read_file: "Read",
  write_file: "Write",
  edit_file: "Edit",
  list_dir: "LS",
  glob: "Glob",
  grep: "Grep",
  web_fetch: "WebFetch",
};

/** What follows the server prefix, or null for any other tool. */
function leafOf(name: string): string | null {
  return PREFIX.exec(name.trim())?.[1].toLowerCase() ?? null;
}

/** The leaf of one of the eight tools that have a built-in twin. */
export function opmanToolLeaf(name: string): string | null {
  const leaf = leafOf(name);
  return leaf !== null && Object.hasOwn(NATIVE, leaf) ? leaf : null;
}

/**
 * The name to show for an opman-tools call: the built-in's for the eight, the plain leaf
 * words for the rest (`wait`, `scheduler_list`). Null when it is not an opman-tools call.
 */
export function opmanDisplayName(name: string): string | null {
  const leaf = leafOf(name);
  if (leaf === null) return null;
  if (Object.hasOwn(NATIVE, leaf)) return NATIVE[leaf];
  const words = leaf.replace(/_+/g, " ").trim();
  return words ? words.charAt(0).toUpperCase() + words.slice(1) : null;
}

type Json = Record<string, unknown>;

function parse(output: string): Json | null {
  try {
    const value: unknown = JSON.parse(output);
    return value !== null && typeof value === "object" && !Array.isArray(value) ? (value as Json) : null;
  } catch {
    return null;
  }
}

const text = (value: unknown): string => (typeof value === "string" ? value : "");
const list = (value: unknown): Json[] => (Array.isArray(value) ? (value.filter((v) => v && typeof v === "object") as Json[]) : []);

function bash(data: Json): string {
  const job = data.job as Json | undefined;
  if (data.background === true && job) {
    return [
      `Still running, so it moved to the background (job ${text(job.id)}, pid ${String(job.pid ?? "?")}).`,
      `stdout: ${text(job.stdout_file)}`,
      `stderr: ${text(job.stderr_file)}`,
      `result when done: ${text(job.exit_file)}`,
    ].join("\n");
  }
  const parts = [text(data.stdout).trimEnd()];
  const stderr = text(data.stderr).trimEnd();
  if (stderr) parts.push(`[stderr]\n${stderr}`);
  if (data.timed_out === true) parts.push("[timed out]");
  else if (typeof data.exit_code === "number" && data.exit_code !== 0) parts.push(`[exit code ${data.exit_code}]`);
  else if (typeof data.signal === "number") parts.push(`[killed by signal ${data.signal}]`);
  return parts.filter(Boolean).join("\n");
}

function read(data: Json): string {
  if (data.binary === true) return `Binary file, ${String(data.bytes ?? "unknown")} bytes.`;
  const more = data.has_more === true ? `\n[more lines after ${String(data.end_line)}]` : "";
  return `<path>${text(data.path)}</path>\n<content>${text(data.content)}${more}</content>`;
}

function grep(data: Json): string {
  const rows = list(data.results);
  const lines = rows.map((row) => {
    if (data.mode === "content") return `${text(row.path)}:${String(row.line ?? "")}:${text(row.text)}`;
    if (data.mode === "count") return `${text(row.path)}: ${String(row.count ?? 0)}`;
    return text(row.path);
  });
  return (data.truncated === true ? [...lines, "[more results not shown]"] : lines).join("\n");
}

function listDir(data: Json): string {
  return list(data.entries)
    .map((entry) => `${text(entry.name)}${entry.kind === "dir" ? "/" : ""}`)
    .join("\n");
}

function write(data: Json): string {
  const verb = data.created === true ? "Created" : "Wrote";
  return `${verb} ${text(data.path)} (${String(data.bytes ?? 0)} bytes)`;
}

function edit(data: Json): string {
  const count = Number(data.replacements ?? 0);
  return `Edited ${text(data.path)}: ${count} ${count === 1 ? "change" : "changes"}\n${text(data.diff)}`.trimEnd();
}

const CONVERTERS: Readonly<Partial<Record<string, (data: Json) => string>>> = {
  bash,
  read_file: read,
  write_file: write,
  edit_file: edit,
  list_dir: listDir,
  glob: (data) => (Array.isArray(data.files) ? data.files.map(String).join("\n") : ""),
  grep,
  web_fetch: (data) => text(data.content),
};

/** The plain text for a tool's JSON result. A result that is not that JSON is left alone. */
export function readableOutput(leaf: string, output: string): string {
  const convert = CONVERTERS[leaf];
  const data = convert ? parse(output) : null;
  return data && convert ? convert(data) : output;
}

/** The same call with its result as text, ready for the built-in cards. */
export function withReadableOutput(part: MessagePart): MessagePart {
  const name = part.tool || part.toolName;
  const leaf = typeof name === "string" ? opmanToolLeaf(name) : null;
  const output = part.state?.output;
  if (leaf === null || typeof output !== "string" || !part.state) return part;
  const converted = readableOutput(leaf, output);
  return converted === output ? part : { ...part, state: { ...part.state, output: converted } };
}
