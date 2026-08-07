/**
 * The per-session subscription store.
 *
 * `useSSE` keeps one `messages` array bound to the active session, which is
 * correct for a shell with one chat in it and useless for a workspace with
 * three. The data was already there — `eventHandler` writes foreign sessions'
 * events into the LRU cache — but nothing could *subscribe* to a session it
 * wasn't looking at.
 *
 * This is that missing half, and deliberately nothing more: it opens no
 * connections and owns no fetching. `useSSE` stays the single reader of the
 * event stream and pushes snapshots in here; panes read them out. Two sources
 * of truth for a live transcript is a bug factory, so there is still only one.
 *
 * Subscription also *pins*: a session someone is watching must not be evicted
 * from the LRU cache under them, and one that isn't loaded yet has to be
 * fetched. Both are answered by `pinned()` and `onDemand()`, which `useSSE`
 * consults — the store states the requirement, the owner satisfies it.
 */

import { SESSION_IDLE, type SessionStatus } from "./types";
import type { SessionStats } from "../../api";
import type { Message } from "../../types";

export interface SessionView {
  readonly messages: readonly Message[];
  readonly stats: SessionStats | null;
  readonly status: SessionStatus;
  /** True between subscribing to a cold session and its first snapshot. */
  readonly loading: boolean;
  /** Whether the server has older messages than the ones held here. */
  readonly hasOlder: boolean;
  /** Total messages in the session, as reported by the server. */
  readonly total: number;
}

const EMPTY_VIEW: SessionView = {
  messages: [],
  stats: null,
  status: SESSION_IDLE,
  loading: true,
  hasOlder: false,
  total: 0,
};

type Listener = () => void;

interface Entry {
  view: SessionView;
  readonly listeners: Set<Listener>;
}

const entries = new Map<string, Entry>();
let demandHandler: ((sessionId: string) => void) | null = null;
let olderLoader: ((sessionId: string) => Promise<boolean>) | null = null;

function entryFor(sessionId: string): Entry {
  const existing = entries.get(sessionId);
  if (existing) return existing;
  const created: Entry = { view: EMPTY_VIEW, listeners: new Set() };
  entries.set(sessionId, created);
  return created;
}

/**
 * Watch a session. Returns the unsubscribe.
 *
 * Refcounted through the listener set, because two panes may show the same
 * session and the first one to close must not unpin it for the other.
 */
export function subscribeSession(sessionId: string, listener: Listener): () => void {
  const entry = entryFor(sessionId);
  const cold = entry.listeners.size === 0;
  entry.listeners.add(listener);
  if (cold) demandHandler?.(sessionId);

  return () => {
    entry.listeners.delete(listener);
    // Keep the last snapshot: a pane that closes and reopens should not flash
    // an empty transcript, and the entry is a few objects, not a transcript
    // copy — the messages are the same array `useSSE` already holds.
    if (entry.listeners.size === 0 && entry.view === EMPTY_VIEW) entries.delete(sessionId);
  };
}

export function getSessionView(sessionId: string): SessionView {
  return entries.get(sessionId)?.view ?? EMPTY_VIEW;
}

/**
 * Publish a new snapshot and wake that session's subscribers.
 *
 * The identity check matters: `useSSE` re-publishes on every flush, and
 * `useSyncExternalStore` compares snapshots by reference, so handing back an
 * equal-but-new object would re-render every pane on every keystroke of
 * streamed output for sessions whose data did not actually change.
 */
export function publishSession(sessionId: string, view: SessionView): void {
  const entry = entries.get(sessionId);
  if (!entry) return;
  if (
    entry.view.messages === view.messages &&
    entry.view.stats === view.stats &&
    entry.view.status === view.status &&
    entry.view.loading === view.loading &&
    entry.view.hasOlder === view.hasOlder &&
    entry.view.total === view.total
  ) {
    return;
  }
  entry.view = view;
  for (const listener of entry.listeners) listener();
}

/** Sessions with at least one subscriber — these must survive LRU eviction. */
export function pinnedSessions(): ReadonlySet<string> {
  const pinned = new Set<string>();
  for (const [sessionId, entry] of entries) {
    if (entry.listeners.size > 0) pinned.add(sessionId);
  }
  return pinned;
}

export function isSessionPinned(sessionId: string): boolean {
  const entry = entries.get(sessionId);
  return entry !== undefined && entry.listeners.size > 0;
}

/**
 * Register who hydrates a session the first time someone watches it. `useSSE`
 * owns the fetch; the store only says when one is needed.
 */
export function setSessionDemandHandler(handler: ((sessionId: string) => void) | null): void {
  demandHandler = handler;
}

/**
 * Register who pages a session backwards. Same split as the demand handler:
 * `useSSE` owns the fetch and the message map, the store only routes the ask so
 * a pane can page a session it is watching without being the active one.
 */
export function setSessionOlderLoader(
  loader: ((sessionId: string) => Promise<boolean>) | null,
): void {
  olderLoader = loader;
}

/** Load one page of older messages. Resolves to whether more remain. */
export function loadOlderIn(sessionId: string): Promise<boolean> {
  return olderLoader?.(sessionId) ?? Promise.resolve(false);
}

/** Forget a session entirely — it was deleted upstream. */
export function dropSession(sessionId: string): void {
  const entry = entries.get(sessionId);
  if (!entry) return;
  entry.view = EMPTY_VIEW;
  for (const listener of entry.listeners) listener();
  if (entry.listeners.size === 0) entries.delete(sessionId);
}

/** Test seam. Never called by the app. */
export function resetSessionStore(): void {
  entries.clear();
  demandHandler = null;
  olderLoader = null;
}

export { EMPTY_VIEW };
