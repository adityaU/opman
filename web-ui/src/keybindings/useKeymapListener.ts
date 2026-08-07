import { useCallback, useEffect, useRef, useState } from "react";
import { stepFromEvent } from "./chord";
import { useKeymapContext } from "./KeymapContext";
import type { Keymap, MatchContext } from "./matcher";
import type { ChordStep } from "./types";
import { whenEvaluator } from "./when";

/**
 * The live keyboard listener.
 *
 * Registered in the capture phase on `document`, so a chord resolves before any
 * surface-local handler sees the event. Pending chord state lives here rather
 * than in the matcher, which stays pure.
 */

/** Keys that only modify: pressing one alone must not cancel a pending chord. */
const MODIFIER_KEYS = new Set(["control", "shift", "alt", "meta", "capslock"]);

function isTextInput(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  if (tag === "TEXTAREA") return true;
  if (tag !== "INPUT") return false;
  // Checkboxes and buttons are inputs that do not swallow keystrokes.
  const type = (target as HTMLInputElement).type;
  return type !== "checkbox" && type !== "radio" && type !== "button" && type !== "submit";
}

type WhichKeyControl = "cancel" | "ascend" | "reveal";

/**
 * Escape, Backspace and `?` steer the hint panel while a chord is pending —
 * but only when the keymap does not want them. A namespace that genuinely binds
 * `?` keeps it, because a binding the user configured should beat a convenience.
 */
function whichKeyControl(
  key: string,
  keymap: Keymap,
  steps: readonly ChordStep[],
  ctx: MatchContext,
): WhichKeyControl | undefined {
  const control =
    key === "Escape" ? "cancel" : key === "Backspace" ? "ascend" : key === "?" ? "reveal" : undefined;
  if (!control) return undefined;

  const claimed = keymap
    .continuations(steps, ctx)
    .some((binding) => binding.seq[steps.length]?.key === key.toLowerCase());
  return claimed ? undefined : control;
}

export interface PendingChord {
  readonly steps: readonly ChordStep[];
  /** When the chord started, for the which-key delay. */
  readonly startedAt: number;
}

export interface KeymapListenerState {
  readonly pending: PendingChord | undefined;
  /** True when the user asked for the hints before the delay elapsed. */
  readonly revealed: boolean;
  /** Clear a pending chord — the which-key panel's Escape. */
  readonly cancel: () => void;
  /** Drop the last step — the which-key panel's Backspace. */
  readonly ascend: () => void;
}

export function useKeymapListener(): KeymapListenerState {
  const { keymap, mode, config, context, runCommand } = useKeymapContext();
  const [pending, setPending] = useState<PendingChord | undefined>();
  const [revealed, setRevealed] = useState(false);

  // The listener is registered once; everything it reads goes through a ref so
  // that a context change does not detach and reattach the handler.
  const latest = useRef({ keymap, mode, context, runCommand, pending });
  latest.current = { keymap, mode, context, runCommand, pending };

  const cancel = useCallback(() => {
    setPending(undefined);
    setRevealed(false);
  }, []);

  const ascend = useCallback(
    () =>
      setPending((current) => {
        if (!current || current.steps.length <= 1) {
          setRevealed(false);
          return undefined;
        }
        return { ...current, steps: current.steps.slice(0, -1) };
      }),
    [],
  );

  useEffect(() => {
    const timeoutMs = config.chordTimeoutMs;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const clearTimer = () => {
      if (timer !== undefined) clearTimeout(timer);
      timer = undefined;
    };

    function onKeyDown(event: KeyboardEvent) {
      if (MODIFIER_KEYS.has(event.key.toLowerCase())) return;

      const { keymap: map, mode: current, context: ctx, runCommand: run } = latest.current;
      const steps = latest.current.pending?.steps ?? [];

      // `textInput` is read from the event target rather than from the
      // published context, and then handed to the `when` evaluator as a context
      // key of its own. That is what lets a binding opt *out* of the insert
      // guard it would otherwise pass on the strength of its modifier —
      // `ctrl+h` must not fire while a shell is waiting for a Backspace.
      const textInput = isTextInput(event.target);
      const matchContext: MatchContext = {
        mode: current,
        textInput,
        isTrue: whenEvaluator({ ...ctx, textInput }),
      };

      // While a chord is pending, three keys belong to the hint panel rather
      // than to the keymap. They are checked before matching so that Escape
      // cancels the chord instead of running the global Escape command.
      if (steps.length > 0) {
        const control = whichKeyControl(event.key, map, steps, matchContext);
        if (control) {
          event.preventDefault();
          event.stopPropagation();
          clearTimer();
          if (control === "cancel") {
            setPending(undefined);
            setRevealed(false);
            return;
          }
          if (control === "ascend") {
            ascend();
            return;
          }
          setRevealed(true);
          return;
        }
      }

      const result = map.match(steps, stepFromEvent(event), matchContext);

      if (result.type === "none") {
        // Only swallow the key if it was continuing a chord: an unbound key with
        // nothing pending belongs to whatever has focus.
        if (steps.length > 0) {
          event.preventDefault();
          event.stopPropagation();
          clearTimer();
          setPending(undefined);
        }
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      clearTimer();

      if (result.type === "pending") {
        setPending({ steps: result.steps, startedAt: Date.now() });
        setRevealed(false);
        timer = setTimeout(() => setPending(undefined), timeoutMs);
        return;
      }

      setPending(undefined);
      setRevealed(false);
      run(result.command);
    }

    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.removeEventListener("keydown", onKeyDown, true);
      clearTimer();
    };
  }, [config.chordTimeoutMs, ascend]);

  return { pending, revealed, cancel, ascend };
}
