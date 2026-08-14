import { useCallback, useEffect, useRef, useState } from "react";

/**
 * The pane header, on demand.
 *
 * It used to be a standing preference revealed on hover, which in practice
 * meant it was on screen whenever the pointer was in the pane — the whole time.
 * What survives is the permanent pane-menu button in the corner; the header
 * itself is now peeked with a chord and withdraws on its own, so it costs the
 * pane no space and needs no state to persist.
 *
 * Peeking again while it is up restarts the countdown rather than toggling it
 * away: the chord means "show me", and a second press is what someone reaching
 * for a control does, not a request to hide it.
 */

const PEEK_MS = 5_000;

export interface HeaderPeek {
  readonly peeking: boolean;
  readonly peek: () => void;
}

export function useHeaderPeek(): HeaderPeek {
  const [peeking, setPeeking] = useState(false);
  const timer = useRef<number | null>(null);

  const clear = useCallback(() => {
    if (timer.current === null) return;
    window.clearTimeout(timer.current);
    timer.current = null;
  }, []);

  const peek = useCallback(() => {
    clear();
    setPeeking(true);
    timer.current = window.setTimeout(() => {
      timer.current = null;
      setPeeking(false);
    }, PEEK_MS);
  }, [clear]);

  useEffect(() => clear, [clear]);

  return { peeking, peek };
}
