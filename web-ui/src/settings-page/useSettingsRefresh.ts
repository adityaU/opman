import { useEffect, useRef } from "react";

export const SETTINGS_CHANGED = "opman:settings-changed";

/** Refresh the affected list while preserving other Settings forms and drafts. */
export function useSettingsRefresh(section: string, refresh: () => unknown): void {
  const latest = useRef(refresh);
  latest.current = refresh;
  useEffect(() => {
    const changed = (event: Event) => {
      const detail: unknown = (event as CustomEvent).detail;
      if (detail && typeof detail === "object" && "section" in detail && detail.section === section) {
        latest.current();
      }
    };
    window.addEventListener(SETTINGS_CHANGED, changed);
    return () => window.removeEventListener(SETTINGS_CHANGED, changed);
  }, [section]);
}
