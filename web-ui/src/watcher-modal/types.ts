import type { WatcherSessionEntry } from "../api";

export interface WatcherModalProps {
  onClose: () => void;
  activeSessionId: string | null;
}

export type SessionGroup = "current" | "watched" | "active" | "other";

export const GROUP_LABELS: Record<SessionGroup, string> = {
  current: "Current Session",
  watched: "Watched",
  active: "Active Sessions",
  other: "Other Sessions",
};

export function groupSessions(
  sessions: WatcherSessionEntry[]
): Record<SessionGroup, WatcherSessionEntry[]> {
  const groups: Record<SessionGroup, WatcherSessionEntry[]> = {
    current: [],
    watched: [],
    active: [],
    other: [],
  };
  for (const s of sessions) {
    if (s.is_current) groups.current.push(s);
    else if (s.has_watcher) groups.watched.push(s);
    else if (s.is_active) groups.active.push(s);
    else groups.other.push(s);
  }
  return groups;
}
