export interface CappedText {
  text: string;
  truncated: boolean;
  bytes: number;
}

export interface InnerCall {
  tool: string;
  input: CappedText;
  output: CappedText;
  ok: boolean;
  finished: boolean;
  durationMs: number;
}

export interface CodeResult {
  output: string;
  calls: InnerCall[];
}

const OPEN = "<opman-code-calls>";
const CLOSE = "</opman-code-calls>";

export function isCodeModeTool(name: string): boolean {
  return /^(?:mcp__)?code(?:__|_)execute$/i.test(name)
    || /^mcp__opman[-_]code__execute$/i.test(name);
}

function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function capped(value: unknown): value is CappedText {
  return object(value) && typeof value.text === "string"
    && typeof value.truncated === "boolean" && typeof value.bytes === "number";
}

function call(value: unknown): value is InnerCall {
  return object(value) && typeof value.tool === "string" && capped(value.input)
    && capped(value.output) && typeof value.ok === "boolean"
    && typeof value.finished === "boolean" && typeof value.durationMs === "number";
}

function calls(value: unknown): InnerCall[] | null {
  if (!object(value) || value.version !== 1 || !Array.isArray(value.calls)) return null;
  if (!value.calls.every(call)) return null;
  return value.calls;
}

/** Accept plain text, MCP results, and content arrays used by different runners. */
export function parseCodeResult(raw: string, metadata?: unknown): CodeResult {
  let output = raw;
  let recorded = calls(metadata);
  try {
    const parsed: unknown = JSON.parse(raw);
    const content = object(parsed) ? parsed.content : parsed;
    if (Array.isArray(content)) {
      output = content.filter(object).filter(block => block.type === "text")
        .map(block => typeof block.text === "string" ? block.text : "").join("\n");
    }
    if (object(parsed)) recorded = calls(parsed.structuredContent) ?? recorded;
  } catch { /* Plain output is the usual ACP representation. */ }

  const start = output.indexOf(OPEN);
  const end = start < 0 ? -1 : output.indexOf(CLOSE, start + OPEN.length);
  if (start < 0 || end < 0) return { output, calls: recorded ?? [] };
  try {
    const inner: unknown = JSON.parse(output.slice(start + OPEN.length, end));
    recorded = calls(inner) ?? recorded;
    if (recorded === null) return { output, calls: [] };
    output = `${output.slice(0, start)}${output.slice(end + CLOSE.length)}`.trim();
  } catch { /* Preserve malformed output for inspection. */ }
  return { output, calls: recorded ?? [] };
}
