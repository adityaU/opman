import { guessLanguage } from "./helpers";

// Match complete terminal controls, plus an unfinished escape at the streaming tail.
export const TERMINAL_CONTROL = /\x1b\][^\x07\x1b]*(?:\x07|\x1b\\|$)|\x1b\[[0-?]*[ -/]*[@-~]|\x1b(?:\[[0-?]*[ -/]*|\][^\x07\x1b]*)$/g;

export function stripTerminalControls(output: string): string {
  return output.replace(TERMINAL_CONTROL, "");
}

/** Infer source output only for simple file-display commands, never pipelines. */
export function bashOutputLanguage(output: string, command: string): string {
  if (/^(?:diff --git |--- .+\n\+\+\+ |@@ -\d)/m.test(output)) return "diff";
  if (output.length <= 200_000 && /^\s*[\[{]/.test(output)) {
    try { JSON.parse(output); return "json"; } catch { /* A log can start with a bracket. */ }
  }
  if (/[|;&<>\n]/.test(command)) return "bash";
  const tokens = command.match(/"[^"]*"|'[^']*'|\S+/g) ?? [];
  if (!/^(?:cat|head|tail|bat)$/.test(tokens[0] ?? "")) return "bash";
  const path = tokens.at(-1)?.replace(/^(['"])(.*)\1$/, "$2");
  if (!path || path.startsWith("-")) return "bash";
  const language = guessLanguage(path);
  return language === "text" ? "bash" : language;
}
