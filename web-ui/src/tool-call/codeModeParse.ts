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

const RECORDS = /<opman-code-calls>([\s\S]*?)<\/opman-code-calls>/g;

export function isCodeModeTool(name: string): boolean {
  return /^(?:mcp__)?(?:opman[-_]code|code)(?:__|[_.])execute$/i.test(name.trim())
    || isNodeReplTool(name);
}

export function isNodeReplTool(name: string): boolean {
  return /^(?:mcp__)?node[-_]repl(?:__|[_.])js$/i.test(name.trim());
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
    const envelope = object(parsed) && object(parsed.result) ? parsed.result : parsed;
    const content = object(envelope) ? envelope.content : envelope;
    if (Array.isArray(content)) {
      output = content.filter(object).filter(block => block.type === "text")
        .map(block => typeof block.text === "string" ? block.text : "").join("\n");
    }
    if (object(envelope)) recorded = calls(envelope.structuredContent) ?? recorded;
  } catch { /* Plain output is the usual ACP representation. */ }

  const markedCalls: InnerCall[] = [];
  let found = false;
  output = output.replace(RECORDS, (block, json: string) => {
    try {
      const inner = calls(JSON.parse(json));
      if (inner === null) return block;
      markedCalls.push(...inner);
      found = true;
      return "";
    } catch { return block; }
  });
  if (found) {
    recorded = markedCalls;
    output = output.trim();
  }
  return { output, calls: recorded ?? [] };
}
