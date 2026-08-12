import type { ClientMsg as EditClientMsg, ControlMsg as EditControlMsg } from "../nvim/edit/wire";

export interface NvimUiUrlOptions {
  readonly sessionId?: string | null;
  readonly projectIdx?: number | null;
  readonly location?: Pick<Location, "protocol" | "host">;
}

/** Build the same-origin WebSocket endpoint; auth is carried by the cookie. */
export function buildNvimUiUrl(options: NvimUiUrlOptions = {}): string {
  const location = options.location ?? (typeof window !== "undefined" ? window.location : null);
  const protocol = location?.protocol === "https:" ? "wss:" : "ws:";
  const host = location?.host ?? "localhost";
  const query = new URLSearchParams();
  if (options.sessionId) query.set("session_id", options.sessionId);
  if (options.projectIdx !== undefined && options.projectIdx !== null) query.set("project_idx", String(options.projectIdx));
  const suffix = query.toString();
  return `${protocol}//${host}/api/nvim/ui${suffix ? `?${suffix}` : ""}`;
}

export type NvimEditClientMsg = EditClientMsg;
export type NvimEditControlMsg = EditControlMsg;

/** Send a closed-protocol edit-engine message over an open text socket. */
export function sendNvimEditClientMsg(socket: WebSocket, message: EditClientMsg): boolean {
  if (socket.readyState !== WebSocket.OPEN) return false;
  socket.send(JSON.stringify(message));
  return true;
}

export const EDIT_CLIENT_MSG_TYPES = ["attach", "edit", "input", "input_mouse", "resize", "paste", "command"] as const;
export const EDIT_CONTROL_MSG_TYPES = [
  "ready", "input_ack", "attached", "buffer_changed", "buffer_detached", "resync_required", "state",
  "command_output", "message", "error", "exited", "superseded", "too_slow",
] as const;
