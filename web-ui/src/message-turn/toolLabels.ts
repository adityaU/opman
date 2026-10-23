import type { MessagePart } from "../types";
import { opmanDisplayName } from "../tool-call/opmanTools";
import { isCodeModeTool } from "../tool-call/codeModeParse";

export function isToolPart(part: MessagePart): boolean {
  return part.type === "tool" || part.type === "tool-call" || part.type === "tool_call";
}

function nameOf(part: MessagePart): string | null {
  const raw = part as MessagePart & Record<string, unknown>;
  const name = raw.tool || raw.toolName || raw.tool_name || raw.name || (raw.call as Record<string, unknown> | undefined)?.name;
  return typeof name === "string" && name.trim() ? name : null;
}

export function toolType(part: MessagePart): string {
  const name = nameOf(part);
  if (!name) return "unknown:" + (part.id || part.callID || part.toolCallId || "part");
  if (isCodeModeTool(name)) return "code_mode";
  const normalized = (opmanDisplayName(name) ?? name).trim().toLowerCase().replace(/[^a-z0-9]+/g, "_");
  if (normalized.includes("ui_render") || normalized === "a2ui") {
    return "a2ui:" + (part.id || part.callID || part.toolCallId || name);
  }
  const bashAliases = new Set(["bash", "shell", "sh", "zsh", "exec", "exec_command", "run_command", "command_execution", "terminal"]);
  const leaf = normalized.split("_").filter(Boolean).pop() || normalized;
  return bashAliases.has(normalized) || bashAliases.has(leaf) ? "bash" : normalized;
}

function toolLabel(name: string): string {
  const label = name.replace(/[_-]+/g, " ").trim();
  return label ? label.charAt(0).toUpperCase() + label.slice(1) : "Tool";
}

export function toolDisplayName(part: MessagePart, type: string): string {
  if (type === "bash") return "Bash";
  if (type === "code_mode") return "Code mode";
  const name = nameOf(part);
  if (!name) return toolLabel(type);
  const trimmed = opmanDisplayName(name) ?? name.trim();
  if (/[_\-\s]/.test(trimmed)) return toolLabel(trimmed.toLowerCase());
  if (/[A-Z]/.test(trimmed.slice(1))) return trimmed;
  return trimmed.charAt(0).toUpperCase() + trimmed.slice(1);
}
