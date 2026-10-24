import { useSyncExternalStore } from "react";

const STORAGE_KEY = "opman-show-thinking";
const CHANGE_EVENT = "opman:thinking-visibility-changed";
let sessionFallback: boolean | undefined;

export function loadThinkingVisibility(): boolean {
  if (sessionFallback !== undefined) return sessionFallback;
  try {
    return localStorage.getItem(STORAGE_KEY) === "true";
  } catch {
    return false;
  }
}

export function saveThinkingVisibility(show: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY, String(show));
    sessionFallback = undefined;
  } catch {
    // The preference still applies for this session when storage is unavailable.
    sessionFallback = show;
  }
  window.dispatchEvent(new Event(CHANGE_EVENT));
}

function subscribe(onChange: () => void): () => void {
  window.addEventListener(CHANGE_EVENT, onChange);
  window.addEventListener("storage", onChange);
  return () => {
    window.removeEventListener(CHANGE_EVENT, onChange);
    window.removeEventListener("storage", onChange);
  };
}

export function useThinkingVisibility(): boolean {
  return useSyncExternalStore(subscribe, loadThinkingVisibility, () => false);
}
