import { useSyncExternalStore } from "react";

const QUERY = "(prefers-reduced-motion: reduce)";

function media(): MediaQueryList | null {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia(QUERY)
    : null;
}

function subscribe(onChange: () => void): () => void {
  const list = media();
  if (!list) return () => {};
  list.addEventListener?.("change", onChange);
  return () => list.removeEventListener?.("change", onChange);
}

const read = () => media()?.matches ?? false;

/** Live `prefers-reduced-motion: reduce`. False where it cannot be asked. */
export function usePrefersReducedMotion(): boolean {
  return useSyncExternalStore(subscribe, read, () => false);
}
