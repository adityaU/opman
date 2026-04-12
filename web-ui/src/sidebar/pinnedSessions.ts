// ── Pin persistence via localStorage ─────────────────

const PINNED_KEY = "opman-pinned-sessions";

export function loadPinnedSessions(): Set<string> {
  try {
    const raw = localStorage.getItem(PINNED_KEY);
    if (raw) return new Set(JSON.parse(raw));
  } catch { /* ignore */ }
  return new Set();
}

export function savePinnedSessions(pinned: Set<string>) {
  try {
    localStorage.setItem(PINNED_KEY, JSON.stringify([...pinned]));
  } catch { /* ignore */ }
}
