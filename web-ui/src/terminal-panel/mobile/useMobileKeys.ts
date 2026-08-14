import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { TerminalRuntime } from "../types";
import { writeToPty } from "../writes";
import {
  applyModifiers, nextModifierState, type ModifierState,
} from "./keys";

const EXTRAS_KEY = "opman-term-extra-keys";

function readExtrasOpen(): boolean {
  try {
    return localStorage.getItem(EXTRAS_KEY) === "1";
  } catch {
    return false;
  }
}

export interface MobileKeys {
  ctrl: ModifierState;
  alt: ModifierState;
  extrasOpen: boolean;
  toggleCtrl: () => void;
  toggleAlt: () => void;
  toggleExtras: () => void;
  /** Send a literal sequence, consuming any armed modifier. */
  press: (seq: string) => void;
  /** Raise the soft keyboard by focusing the active terminal. */
  focusTerminal: () => void;
}

/**
 * Sticky modifiers for a touch terminal.
 *
 * A phone cannot hold Ctrl while pressing a letter, so Ctrl and Alt become
 * *states*: tap to arm for the next keystroke, tap again to lock. The arming
 * has to reach the soft keyboard's own output too, not just the on-screen key
 * buttons — so the transform is handed to the terminal's data path through a
 * ref rather than applied only here.
 */
export function useMobileKeys(
  ptyId: string | null,
  runtimeRef: React.MutableRefObject<TerminalRuntime | null>,
  transformRef: React.MutableRefObject<((data: string) => string) | null>,
  enabled: boolean,
): MobileKeys {
  const [ctrl, setCtrl] = useState<ModifierState>("off");
  const [alt, setAlt] = useState<ModifierState>("off");
  const [extrasOpen, setExtrasOpen] = useState(readExtrasOpen);

  // Read at call time: the transform runs from xterm's data callback, which is
  // installed once per shell and must not be re-created on every modifier tap.
  const state = useRef({ ctrl, alt });
  state.current = { ctrl, alt };

  const consume = useCallback(() => {
    setCtrl((c) => (c === "armed" ? "off" : c));
    setAlt((a) => (a === "armed" ? "off" : a));
  }, []);

  // Install the transform for the soft keyboard's own keystrokes.
  useEffect(() => {
    if (!enabled) return;
    transformRef.current = (data: string) => {
      const { ctrl: c, alt: a } = state.current;
      if (c === "off" && a === "off") return data;
      consume();
      return applyModifiers(data, c !== "off", a !== "off");
    };
    return () => {
      transformRef.current = null;
    };
  }, [enabled, transformRef, consume]);

  const focusTerminal = useCallback(() => {
    runtimeRef.current?.term.focus();
  }, [runtimeRef]);

  const press = useCallback(
    (seq: string) => {
      if (!ptyId) return;
      const { ctrl: c, alt: a } = state.current;
      const data = applyModifiers(seq, c !== "off", a !== "off");
      if (c !== "off" || a !== "off") consume();
      writeToPty(ptyId, data);
      // Every key press is a hint that the user wants to keep typing.
      focusTerminal();
      navigator.vibrate?.(8);
    },
    [ptyId, consume, focusTerminal],
  );

  const toggleCtrl = useCallback(() => {
    setCtrl(nextModifierState);
    navigator.vibrate?.(8);
  }, []);

  const toggleAlt = useCallback(() => {
    setAlt(nextModifierState);
    navigator.vibrate?.(8);
  }, []);

  const toggleExtras = useCallback(() => {
    setExtrasOpen((open) => {
      const next = !open;
      try { localStorage.setItem(EXTRAS_KEY, next ? "1" : "0"); } catch { /* ignore */ }
      return next;
    });
  }, []);

  return useMemo(
    () => ({
      ctrl, alt, extrasOpen,
      toggleCtrl, toggleAlt, toggleExtras, press, focusTerminal,
    }),
    [ctrl, alt, extrasOpen, toggleCtrl, toggleAlt, toggleExtras, press, focusTerminal],
  );
}
