import { createPtySSE } from "../api";
import { apiUrl } from "../api/base";
import { writeToPty } from "./writes";

/**
 * One terminal view's line to its PTY.
 *
 * The terminal's own WebSocket (`/pty/ws`) carries raw bytes both ways: a keystroke is
 * one binary frame, output arrives the moment the PTY writes it. That replaced an HTTP
 * POST per keystroke (each waiting for the previous one's reply) and an output stream
 * the server only checked every 50 ms — together most of the lag a user felt typing.
 *
 * The old pair (SSE out, ordered POSTs in) stays as the fallback, for a socket that
 * cannot open or drops: the view keeps working, just slower.
 */
export interface PtyConnection {
  /** Send what the user typed. Ordered, never dropped while the view is attached. */
  send(data: string): void;
  /** Detach. The program on the other end keeps running. */
  close(): void;
}

type Output = (bytes: Uint8Array) => void;

const encoder = new TextEncoder();

function socketUrl(id: string, replay: boolean): string {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const query = `id=${encodeURIComponent(id)}${replay ? "&replay=1" : ""}`;
  return `${protocol}//${window.location.host}${apiUrl(`/pty/ws?${query}`)}`;
}

/** SSE output is base64, so genuine escape bytes survive the text stream. */
function decodeBase64(data: string): Uint8Array {
  const raw = atob(data);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return bytes;
}

function fallback(id: string, replay: boolean, onOutput: Output): PtyConnection {
  const sse = createPtySSE(id, replay);
  sse.addEventListener("output", (event) => {
    try {
      onOutput(decodeBase64((event as MessageEvent<string>).data));
    } catch {
      onOutput(encoder.encode((event as MessageEvent<string>).data));
    }
  });
  return {
    send: (data) => writeToPty(id, data),
    close: () => sse.close(),
  };
}

export function connectPty(id: string, replay: boolean, onOutput: Output): PtyConnection {
  let closed = false;
  let opened = false;
  // Keys typed before the socket opens, sent in order once it does.
  let queued = "";
  let backup: PtyConnection | null = null;
  const socket = new WebSocket(socketUrl(id, replay));
  socket.binaryType = "arraybuffer";

  const useBackup = (replayAgain: boolean) => {
    if (closed || backup) return;
    // Only a socket that never opened may replay; one that delivered output already
    // painted it, and replaying would print it twice.
    backup = fallback(id, replayAgain, onOutput);
    if (queued) backup.send(queued);
    queued = "";
  };

  socket.addEventListener("open", () => {
    opened = true;
    if (queued) socket.send(encoder.encode(queued));
    queued = "";
  });
  socket.addEventListener("message", (event) => {
    const data = event.data;
    onOutput(typeof data === "string" ? encoder.encode(data) : new Uint8Array(data as ArrayBuffer));
  });
  socket.addEventListener("close", () => useBackup(replay && !opened));

  return {
    send(data) {
      if (backup) return backup.send(data);
      if (socket.readyState === WebSocket.OPEN) socket.send(encoder.encode(data));
      else queued += data;
    },
    close() {
      closed = true;
      socket.close();
      backup?.close();
    },
  };
}
