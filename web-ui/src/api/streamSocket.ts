/**
 * EventSource over the shared stream socket.
 *
 * `MuxEventSource` behaves like the browser's `EventSource` for everything the app
 * uses — named and `message` listeners, `onopen`/`onmessage`/`onerror`, `readyState`,
 * `close()` — but subscribes on its server's one {@link StreamHub} socket instead of
 * holding an HTTP connection of its own. See `streamHub.ts` for why.
 *
 * Reconnect semantics follow EventSource so existing recovery code keeps working: a
 * dropped socket or a finished stream fires `error` with `readyState` CONNECTING and
 * then `open` again once resubscribed; a refused stream (non-200, not SSE) fires
 * `error` with `readyState` CLOSED and stays closed.
 */
import { detectBase } from "./base";
import { StreamHub, type StreamSink } from "./streamHub";

const hubs = new Map<string, StreamHub>();

function hubFor(base: string): StreamHub {
  let hub = hubs.get(base);
  if (!hub) {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    hub = new StreamHub(`${protocol}//${window.location.host}${base}/api/stream/ws`);
    hubs.set(base, hub);
  }
  return hub;
}

/** Drop every hub. Tests only — live sockets are left to their own sources. */
export function resetStreamHubs(): void {
  hubs.clear();
}

/** `{ base, path }` for a same-origin `${base}/api/...` URL, else `null`. */
export function muxTarget(url: string): { base: string; path: string } | null {
  let parsed: URL;
  try {
    parsed = new URL(url, window.location.href);
  } catch {
    return null;
  }
  if (parsed.origin !== window.location.origin) return null;
  const base = detectBase(parsed.pathname);
  const path = parsed.pathname.slice(base.length);
  if (!path.startsWith("/api/")) return null;
  return { base, path: `${path}${parsed.search}` };
}

type Handler = ((this: EventSource, ev: Event) => unknown) | null;
type MessageHandler = ((this: EventSource, ev: MessageEvent) => unknown) | null;

export class MuxEventSource extends EventTarget implements StreamSink {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSED = 2;
  readonly CONNECTING = 0;
  readonly OPEN = 1;
  readonly CLOSED = 2;

  readonly url: string;
  readonly withCredentials = true;
  readonly path: string;
  readyState: number = MuxEventSource.CONNECTING;
  onopen: Handler = null;
  onmessage: MessageHandler = null;
  onerror: Handler = null;

  private readonly hub: StreamHub;
  private readonly id: number;
  private lastEventId = "";

  /** `url` is an `apiUrl(...)` result: `${BASE}/api/...`. */
  constructor(url: string) {
    super();
    const target = muxTarget(url);
    if (!target) throw new TypeError(`not a stream path on this origin: ${url}`);
    this.url = new URL(url, window.location.href).href;
    this.path = target.path;
    this.hub = hubFor(target.base);
    this.id = this.hub.subscribe(this);
  }

  close(): void {
    if (this.readyState === MuxEventSource.CLOSED) return;
    this.readyState = MuxEventSource.CLOSED;
    this.hub.unsubscribe(this.id);
  }

  onStreamOpen(): void {
    if (this.readyState === MuxEventSource.CLOSED) return;
    this.readyState = MuxEventSource.OPEN;
    this.fire(new Event("open"));
  }

  onStreamEvent(event: string, data: string, lastId: string | undefined): void {
    if (this.readyState === MuxEventSource.CLOSED) return;
    if (lastId !== undefined) this.lastEventId = lastId;
    this.fire(
      new MessageEvent(event, { data, lastEventId: this.lastEventId, origin: window.location.origin }),
    );
  }

  onStreamEnd(status: number | undefined): void {
    if (this.readyState === MuxEventSource.CLOSED) return;
    // Refused: closed for good. Finished: the hub asks again, like EventSource's retry.
    this.readyState = status === undefined ? MuxEventSource.CONNECTING : MuxEventSource.CLOSED;
    this.fire(new Event("error"));
  }

  onStreamInterrupted(): void {
    if (this.readyState === MuxEventSource.CLOSED) return;
    // Like EventSource, every failed attempt is an `error` while still CONNECTING.
    this.readyState = MuxEventSource.CONNECTING;
    this.fire(new Event("error"));
  }

  private fire(event: Event): void {
    const self = this as unknown as EventSource;
    if (event.type === "open") this.onopen?.call(self, event);
    else if (event.type === "error") this.onerror?.call(self, event);
    else if (event.type === "message") this.onmessage?.call(self, event as MessageEvent);
    this.dispatchEvent(event);
  }
}

/**
 * Open an event stream for an `apiUrl(...)` URL.
 *
 * Every stream of this frontend instance shares one WebSocket. Falls back to a native
 * EventSource where WebSocket is unavailable or the URL is not one of this origin's
 * `/api/` paths.
 */
export function openEventStream(url: string): EventSource {
  if (typeof WebSocket === "undefined" || !muxTarget(url)) return new EventSource(url);
  return new MuxEventSource(url) as unknown as EventSource;
}
