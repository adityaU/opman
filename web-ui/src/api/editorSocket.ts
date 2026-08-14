import { decode, encode } from "@msgpack/msgpack";

/**
 * The editor's binary channel.
 *
 * Every language-server query for the editor rides one WebSocket carrying
 * MessagePack frames, multiplexed by request id. Two reasons, both of which
 * only show up under load:
 *
 * A browser gives an origin roughly six HTTP connections. Scrolling through
 * code fires a hover per pointer rest and typing fires a completion per
 * keystroke, so a fast reader or a fast typist puts more than six queries in
 * the air at once — and the surplus waits in the connection pool, behind
 * whatever else the app is doing, before the language server has even seen it.
 *
 * And an HTTP request cannot be taken back. The hover you moved past still
 * runs, still occupies a slot, and still answers. Here a superseded request is
 * withdrawn, so the server spends its time on the query somebody is waiting for.
 */

export type EditorOp =
  | "hover" | "goto" | "references" | "rename" | "format" | "completion" | "diagnostics";

interface Pending {
  readonly resolve: (value: unknown) => void;
  readonly reject: (error: Error) => void;
}

interface Frame {
  readonly id: number;
  readonly result?: unknown;
  readonly error?: string;
  readonly event?: string;
  readonly payload?: unknown;
}

/** Raised when a request is superseded. Callers treat it as "no answer needed". */
export class Superseded extends Error {
  constructor() {
    super("superseded");
    this.name = "Superseded";
  }
}

export type EditorEventHandler = (event: string, payload: unknown) => void;

const RETRY_MS = 1_000;

class EditorSocket {
  private socket: WebSocket | null = null;
  private opening: Promise<WebSocket> | null = null;
  private nextId = 1;
  private readonly pending = new Map<number, Pending>();
  private readonly handlers = new Set<EditorEventHandler>();
  private closed = false;

  onEvent(handler: EditorEventHandler): () => void {
    this.handlers.add(handler);
    return () => this.handlers.delete(handler);
  }

  /**
   * Send a request and resolve with its result.
   *
   * `signal` withdraws it: the socket tells the server to stop, and the promise
   * rejects with `Superseded` so a caller can tell "you moved on" apart from
   * "the language server failed".
   */
  async request(op: EditorOp, payload: unknown, signal?: AbortSignal): Promise<unknown> {
    if (signal?.aborted) throw new Superseded();
    const socket = await this.connect();
    const id = this.nextId++;

    const answer = new Promise<unknown>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });

    socket.send(encode({ id, op, payload }));

    if (!signal) return answer;
    const withdraw = () => {
      const waiting = this.pending.get(id);
      if (!waiting) return;
      this.pending.delete(id);
      this.send({ id: this.nextId++, op: "cancel", payload: { target: id } });
      waiting.reject(new Superseded());
    };
    signal.addEventListener("abort", withdraw, { once: true });
    return answer.finally(() => signal.removeEventListener("abort", withdraw));
  }

  close(): void {
    this.closed = true;
    this.socket?.close();
    this.socket = null;
    this.failAll(new Error("editor channel closed"));
  }

  private send(frame: unknown): void {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(encode(frame));
  }

  private connect(): Promise<WebSocket> {
    if (this.socket?.readyState === WebSocket.OPEN) return Promise.resolve(this.socket);
    if (this.opening) return this.opening;

    this.closed = false;
    this.opening = new Promise<WebSocket>((resolve, reject) => {
      const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
      const socket = new WebSocket(`${protocol}//${window.location.host}/api/editor/ws`);
      socket.binaryType = "arraybuffer";

      socket.addEventListener("open", () => {
        this.socket = socket;
        this.opening = null;
        resolve(socket);
      });
      socket.addEventListener("message", (event) => this.receive(event));
      socket.addEventListener("error", () => {
        this.opening = null;
        reject(new Error("editor channel unavailable"));
      });
      socket.addEventListener("close", () => {
        this.socket = null;
        this.opening = null;
        // Every request still waiting will never be answered by this socket.
        this.failAll(new Error("editor channel closed"));
        if (!this.closed) setTimeout(() => void this.connect().catch(() => {}), RETRY_MS);
      });
    });
    return this.opening;
  }

  private receive(event: MessageEvent): void {
    if (!(event.data instanceof ArrayBuffer)) return;
    const frame = decode(new Uint8Array(event.data)) as Frame;

    // Zero is the server speaking unprompted — a diagnostic set it just
    // published, rather than an answer to anything.
    if (frame.id === 0 && frame.event) {
      for (const handler of this.handlers) handler(frame.event, frame.payload);
      return;
    }

    const waiting = this.pending.get(frame.id);
    if (!waiting) return;
    this.pending.delete(frame.id);
    if (frame.error !== undefined) waiting.reject(new Error(frame.error));
    else waiting.resolve(frame.result);
  }

  private failAll(error: Error): void {
    for (const [, waiting] of this.pending) waiting.reject(error);
    this.pending.clear();
  }
}

/**
 * One socket for the tab, not one per pane: the frames already carry a path, so
 * a second connection would only spend another handshake to say the same thing.
 */
export const editorSocket = new EditorSocket();
