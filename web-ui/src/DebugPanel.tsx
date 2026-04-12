/**
 * DebugLog — module-level ring buffer + DebugPanel component.
 *
 * Provides `dbgLog(msg)` callable from anywhere and a `DebugPanel` React
 * component that renders the live log. Uses `useSyncExternalStore` for
 * zero-context, zero-prop-drilling reactivity.
 */

import React, { useCallback, useSyncExternalStore } from "react";

// ── Ring buffer (module singleton) ───────────────────────────────

const MAX_LINES = 200;

let _lines: string[] = [];
const _listeners = new Set<() => void>();
let _snapshot: string[] = [];

function notify() {
  _snapshot = [..._lines];
  for (const fn of _listeners) fn();
}

function subscribe(cb: () => void): () => void {
  _listeners.add(cb);
  return () => _listeners.delete(cb);
}

function getSnapshot(): string[] {
  return _snapshot;
}

/** Push a debug message to the on-screen overlay log. */
export function dbgLog(msg: string): void {
  if (_lines.length >= MAX_LINES) _lines.shift();
  _lines.push(msg);
  notify();
  // Mirror to devtools console
  // eslint-disable-next-line no-console
  console.info("[dbg]", msg);
}

/** Clear all log lines. */
export function dbgClear(): void {
  _lines = [];
  notify();
}

// ── React component ──────────────────────────────────────────────

export const DebugPanel: React.FC = React.memo(function DebugPanel() {
  const lines = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  const handleClear = useCallback(() => dbgClear(), []);

  return (
    <div className="debug-panel">
      <div className="debug-panel-toolbar">
        <button className="debug-panel-clear" onClick={handleClear} title="Clear log">
          Clear
        </button>
        <span className="debug-panel-count">{lines.length} lines</span>
      </div>
      <div className="debug-panel-log">
        {lines.map((line, i) => (
          <div key={i} className="debug-panel-line">
            <span className="debug-panel-idx">{String(i).padStart(3, " ")}</span>
            <span className="debug-panel-msg">{line}</span>
          </div>
        ))}
      </div>
    </div>
  );
});
