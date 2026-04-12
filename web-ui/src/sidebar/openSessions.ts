// ── Open-sessions persistence via localStorage ──────
// Mirrors pinnedSessions.ts — stores session IDs that
// appear in the "Open Sessions" section at sidebar top.

const OPEN_KEY = "opman-open-sessions";

export function loadOpenSessions(): Set<string> {
  try {
    const raw = localStorage.getItem(OPEN_KEY);
    if (raw) return new Set(JSON.parse(raw));
  } catch { /* ignore */ }
  return new Set();
}

export function saveOpenSessions(open: Set<string>) {
  try {
    localStorage.setItem(OPEN_KEY, JSON.stringify([...open]));
  } catch { /* ignore */ }
}
