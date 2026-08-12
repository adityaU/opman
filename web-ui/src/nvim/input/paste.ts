import type { ClientMsg } from "../wire/control";

/** Build the complete paste notification payload. The backend always uses phase=-1. */
export function pasteMessage(data: string): ClientMsg {
  return { type: "paste", data };
}

export const createPasteMessage = pasteMessage;

/** Consume a browser paste and route it as one nvim_paste operation. */
export function handlePaste(event: ClipboardEvent, send: (message: ClientMsg) => void): boolean {
  const data = event.clipboardData?.getData("text/plain") ?? "";
  if (data.length === 0) return false;
  event.preventDefault();
  send(pasteMessage(data));
  return true;
}

export function pasteText(data: string, send: (message: ClientMsg) => void): void {
  if (data.length > 0) send(pasteMessage(data));
}
