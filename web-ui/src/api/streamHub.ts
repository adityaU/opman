/**
 * The one WebSocket a frontend instance carries all its event streams over.
 *
 * On a plain-HTTP origin the browser allows six HTTP/1.1 connections per origin, shared
 * by the page and every same-origin iframe; each open EventSource holds one forever.
 * WebSockets do not count against that pool, so every stream subscribes on this socket
 * instead (`GET /api/stream/ws`, protocol in `src/web/stream_mux/mod.rs`).
 *
 * The hub opens the socket on the first subscription, resubscribes everything after a
 * drop (with backoff), and closes it shortly after the last subscription goes away.
 */

/** What the hub needs from a subscribed source. */
export interface StreamSink {
  /** The path subscribed, relative to the server: `/api/...`. */
  readonly path: string;
  onStreamOpen(): void;
  onStreamEvent(event: string, data: string, lastId: string | undefined): void;
  /** The stream ended: `status` set means it was refused and will not come back. */
  onStreamEnd(status: number | undefined): void;
  /** The socket dropped; the hub resubscribes once it is back. */
  onStreamInterrupted(): void;
}

interface ServerFrame {
  id?: number;
  op?: "open" | "end" | "ping";
  status?: number;
  event?: string;
  data?: string;
  lastId?: string;
}

/** Close the socket this long after the last subscription left. */
export const IDLE_GRACE_MS = 1500;
/** No frame (the server pings every 15 s) for this long means the socket is dead. */
export const SILENCE_LIMIT_MS = 40_000;
/** How long a finished stream waits before resubscribing — EventSource's default. */
export const RETRY_MS = 3000;
const BACKOFF_MIN_MS = 500;
const BACKOFF_MAX_MS = 10_000;

export class StreamHub {
  private socket: WebSocket | null = null;
  private readonly subs = new Map<number, StreamSink>();
  private nextId = 1;
  private attempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private idleTimer: ReturnType<typeof setTimeout> | null = null;
  private silenceTimer: ReturnType<typeof setInterval> | null = null;
  private lastFrame = 0;

  constructor(private readonly url: string) {}

  /** Subscribe `sink`; returns its id for {@link unsubscribe}. */
  subscribe(sink: StreamSink): number {
    const id = this.nextId++;
    this.subs.set(id, sink);
    this.cancelIdle();
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.send({ op: "sub", id, path: sink.path });
    } else if (!this.socket && !this.reconnectTimer) {
      this.connect();
    }
    return id;
  }

  unsubscribe(id: number): void {
    if (!this.subs.delete(id)) return;
    if (this.socket?.readyState === WebSocket.OPEN) this.send({ op: "unsub", id });
    if (this.subs.size === 0) this.scheduleIdle();
  }

  /** Open subscriptions, for tests and diagnostics. */
  get size(): number {
    return this.subs.size;
  }

  private connect(): void {
    this.reconnectTimer = null;
    if (this.subs.size === 0) return;
    let socket: WebSocket;
    try {
      socket = new WebSocket(this.url);
    } catch {
      this.dropped();
      return;
    }
    this.socket = socket;
    socket.onopen = () => {
      this.attempts = 0;
      this.touch();
      this.watchSilence();
      for (const [id, sink] of this.subs) this.send({ op: "sub", id, path: sink.path });
    };
    socket.onmessage = (message) => {
      this.touch();
      if (typeof message.data === "string") this.route(message.data);
    };
    socket.onclose = () => {
      if (this.socket === socket) this.dropped();
    };
    // A failed socket always closes next; `onclose` does the work.
    socket.onerror = () => {};
  }

  private route(raw: string): void {
    let frame: ServerFrame;
    try {
      frame = JSON.parse(raw) as ServerFrame;
    } catch {
      return;
    }
    if (frame.id === undefined) return;
    const sink = this.subs.get(frame.id);
    if (!sink) return;
    if (frame.op === "open") return sink.onStreamOpen();
    if (frame.op === "end") return this.ended(frame.id, sink, frame.status);
    if (typeof frame.event === "string" && typeof frame.data === "string") {
      sink.onStreamEvent(frame.event, frame.data, frame.lastId);
    }
  }

  /**
   * A refused stream is gone for good; a finished one is asked for again after
   * {@link RETRY_MS} under the same id, as EventSource would reconnect.
   */
  private ended(id: number, sink: StreamSink, status: number | undefined): void {
    if (status !== undefined) {
      this.subs.delete(id);
      sink.onStreamEnd(status);
      if (this.subs.size === 0) this.scheduleIdle();
      return;
    }
    sink.onStreamEnd(undefined);
    setTimeout(() => {
      if (this.subs.get(id) !== sink || this.socket?.readyState !== WebSocket.OPEN) return;
      this.send({ op: "sub", id, path: sink.path });
    }, RETRY_MS);
  }

  /** The socket is gone: tell every stream, then come back with backoff. */
  private dropped(): void {
    this.detachSocket();
    for (const sink of [...this.subs.values()]) sink.onStreamInterrupted();
    if (this.subs.size === 0 || this.reconnectTimer) return;
    const delay = Math.min(BACKOFF_MAX_MS, BACKOFF_MIN_MS * 2 ** this.attempts);
    this.attempts += 1;
    this.reconnectTimer = setTimeout(() => this.connect(), delay * (0.75 + Math.random() * 0.5));
  }

  private detachSocket(): void {
    const socket = this.socket;
    this.socket = null;
    if (this.silenceTimer) clearInterval(this.silenceTimer);
    this.silenceTimer = null;
    if (!socket) return;
    socket.onopen = socket.onmessage = socket.onclose = socket.onerror = null;
    if (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING) {
      socket.close();
    }
  }

  private touch(): void {
    this.lastFrame = Date.now();
  }

  private watchSilence(): void {
    if (this.silenceTimer) clearInterval(this.silenceTimer);
    this.silenceTimer = setInterval(() => {
      if (Date.now() - this.lastFrame > SILENCE_LIMIT_MS) this.dropped();
    }, SILENCE_LIMIT_MS / 4);
  }

  private send(frame: { op: "sub"; id: number; path: string } | { op: "unsub"; id: number }) {
    this.socket?.send(JSON.stringify(frame));
  }

  private scheduleIdle(): void {
    this.cancelIdle();
    this.idleTimer = setTimeout(() => {
      this.idleTimer = null;
      if (this.subs.size > 0) return;
      if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
      this.attempts = 0;
      this.detachSocket();
    }, IDLE_GRACE_MS);
  }

  private cancelIdle(): void {
    if (this.idleTimer) clearTimeout(this.idleTimer);
    this.idleTimer = null;
  }
}
