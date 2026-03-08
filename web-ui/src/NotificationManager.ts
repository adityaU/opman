/**
 * NotificationManager — Browser Notification API utility for session continuity.
 *
 * Uses the browser Notification API (not Web Push / VAPID) so no server-side
 * push infrastructure is needed. Notifications fire when the user has the tab
 * open but may not be looking at it (document.hidden).
 */

export type NotifyEventKind =
  | "session_complete"
  | "permission_request"
  | "question"
  | "watcher_trigger"
  | "file_edit";

export interface NotificationPrefs {
  enabled: boolean;
  session_complete: boolean;
  permission_request: boolean;
  question: boolean;
  watcher_trigger: boolean;
  file_edit: boolean;
}

const PREFS_KEY = "opman_notification_prefs";

const DEFAULT_PREFS: NotificationPrefs = {
  enabled: true,
  session_complete: true,
  permission_request: true,
  question: true,
  watcher_trigger: true,
  file_edit: false, // too noisy by default
};

/** Load prefs from localStorage. */
export function loadNotificationPrefs(): NotificationPrefs {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    if (raw) {
      return { ...DEFAULT_PREFS, ...JSON.parse(raw) };
    }
  } catch {
    // ignore
  }
  return { ...DEFAULT_PREFS };
}

/** Save prefs to localStorage. */
export function saveNotificationPrefs(prefs: NotificationPrefs): void {
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
  } catch {
    // ignore
  }
}

/** Request permission from the browser if not already granted. Returns true if allowed. */
export async function requestNotificationPermission(): Promise<boolean> {
  if (!("Notification" in window)) return false;
  if (Notification.permission === "granted") return true;
  if (Notification.permission === "denied") return false;
  const result = await Notification.requestPermission();
  return result === "granted";
}

/** Check if notifications are supported and permitted. */
export function canNotify(): boolean {
  return "Notification" in window && Notification.permission === "granted";
}

/**
 * Show a browser notification if the tab is hidden and the event kind is enabled.
 * Returns the Notification instance (or null if suppressed).
 */
export function showNotification(
  kind: NotifyEventKind,
  title: string,
  body: string,
  prefs: NotificationPrefs,
  onClick?: () => void
): Notification | null {
  // Only notify when tab is hidden (user is away)
  if (!document.hidden) return null;
  if (!prefs.enabled) return null;
  if (!prefs[kind]) return null;
  if (!canNotify()) return null;

  const icon = "/favicon.ico";
  const tag = `opman-${kind}-${Date.now()}`;

  const notification = new Notification(title, {
    body,
    icon,
    tag,
    silent: false,
  });

  if (onClick) {
    notification.onclick = () => {
      window.focus();
      onClick();
      notification.close();
    };
  }

  // Auto-close after 8 seconds
  setTimeout(() => notification.close(), 8000);

  return notification;
}

/**
 * Generate a stable client ID for this browser tab.
 * Persisted in sessionStorage (per-tab, not shared between tabs).
 */
export function getClientId(): string {
  const KEY = "opman_client_id";
  let id = sessionStorage.getItem(KEY);
  if (!id) {
    id = `web-${crypto.randomUUID?.() ?? Math.random().toString(36).slice(2)}`;
    sessionStorage.setItem(KEY, id);
  }
  return id;
}
