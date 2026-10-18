import type { WidgetState } from "./types";

/**
 * Widgets waiting for the workspace to be on screen.
 *
 * Settings replaces the workspace rather than floating over it, so a page there cannot
 * dispatch into it: nothing is mounted to receive the action. It queues the widget here and
 * leaves; the workspace takes the queue as it mounts and places each one like any other
 * external open.
 */
let queued: WidgetState[] = [];

export function queueWidgetOpen(widget: WidgetState): void {
  queued = [...queued, widget];
}

/** Everything waiting, emptying the queue. */
export function takeQueuedWidgets(): readonly WidgetState[] {
  const taken = queued;
  queued = [];
  return taken;
}
