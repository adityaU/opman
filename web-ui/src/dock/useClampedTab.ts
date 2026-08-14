import { useEffect, useState } from "react";

/**
 * A tab index that never points past the end of the list.
 *
 * Pending requests disappear when they are answered, so an index kept in plain state
 * would leave the dock rendering nothing.
 */
export function useClampedTab(count: number): [number, (index: number) => void] {
  const [active, setActive] = useState(0);

  useEffect(() => {
    setActive((current) => (current < count ? current : Math.max(0, count - 1)));
  }, [count]);

  return [Math.min(active, Math.max(0, count - 1)), setActive];
}
