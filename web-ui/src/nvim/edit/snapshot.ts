import type { BindingSnapshot } from "./decorations";
import type { ControlMsg, ModeShort } from "./wire";

/** A cursor/mode update never clears the status line: the last command's
 *  result stays visible until the next command line opens. */
export function messageToSnapshot(
  message: Extract<ControlMsg, { type: "state" }>,
): Omit<BindingSnapshot, "overlays" | "message"> {
  return {
    mode: message.mode,
    modeShort: canonicalMode(message.mode_short, message.mode),
    cursor: message.cursor,
    visual: message.visual,
    changedtick: message.changedtick,
    connection: { status: "attached" },
  };
}

function canonicalMode(short: string, mode: string): ModeShort {
  if (mode === "i" || mode === "ic" || mode === "ix") return "insert";
  if (mode === "R" || mode === "Rv") return "replace";
  if (mode === "v") return "visual";
  if (mode === "V") return "visual_line";
  if (mode === "\u0016") return "visual_block";
  if (mode.startsWith("o") || mode.startsWith("no")) return "operator_pending";
  if (mode.startsWith("c")) return "command";
  switch (short) {
    case "normal":
    case "insert":
    case "replace":
    case "visual":
    case "visual_line":
    case "visual_block":
    case "operator_pending":
    case "command":
      return short;
  }
  return "normal";
}
