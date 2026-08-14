import { ptyWrite } from "../api";
import { encodeForPty } from "./encode";

/**
 * Ordered writes to a PTY.
 *
 * Every keystroke used to be its own POST, and HTTP gives no ordering across
 * concurrent requests — so fast typing could reach the shell shuffled ("nvim"
 * landing as "nvim" out of order), and a held arrow key turned into an
 * interleaved mess of half escape sequences. One request in flight per PTY,
 * always carrying everything queued since the last one, restores the property
 * a terminal is built on: bytes arrive in the order they were typed.
 *
 * Coalescing is also why holding a key stays cheap — a burst of repeats
 * becomes one request, not a request per repeat.
 */

interface WriteQueue {
  buffer: string;
  flushing: boolean;
}

const queues = new Map<string, WriteQueue>();

export function writeToPty(id: string, data: string): void {
  let queue = queues.get(id);
  if (!queue) {
    queue = { buffer: "", flushing: false };
    queues.set(id, queue);
  }
  queue.buffer += data;
  if (queue.flushing) return;
  queue.flushing = true;
  void flush(id, queue);
}

async function flush(id: string, queue: WriteQueue): Promise<void> {
  while (queue.buffer) {
    const chunk = queue.buffer;
    queue.buffer = "";
    try {
      await ptyWrite(id, encodeForPty(chunk));
    } catch {
      // The PTY is gone or the server hiccuped. Dropping the chunk is the
      // right failure: replaying stale keystrokes into a shell later would be
      // worse than losing them, and the SSE stream shows the truth either way.
    }
  }
  queue.flushing = false;
  queues.delete(id);
}
