// ── Open-sessions persistence via localStorage ──────
// Mirrors pinnedSessions.ts — stores session IDs that
// appear in the "Open Sessions" section at sidebar top.

const OPEN_KEY = "opman-open-sessions";
export const OPEN_SESSION_MAX_AGE_MS = 2 * 24 * 60 * 60 * 1000;

function epochMs(timestamp: number): number {
  return timestamp > 10_000_000_000 ? timestamp : timestamp * 1000;
}

export function isOpenSessionFresh(updated: number, now = Date.now()): boolean {
  if (!updated) return false;
  return now - epochMs(updated) <= OPEN_SESSION_MAX_AGE_MS;
}

export function pruneOpenSessions(open: Set<string>, projects: { sessions: { id: string; time: { updated: number } }[] }[]): Set<string> {
  const updatedById = new Map<string, number>();
  for (const project of projects) {
    for (const session of project.sessions) updatedById.set(session.id, session.time.updated);
  }
  const fresh = new Set<string>();
  for (const sessionId of open) {
    if (isOpenSessionFresh(updatedById.get(sessionId) || 0)) fresh.add(sessionId);
  }
  return fresh;
}

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
