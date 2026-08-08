import {
  cloneElement,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import type { HTMLAttributes, ReactElement, ReactNode } from "react";
import { createPortal } from "react-dom";
import type { CommandId } from "../types";
import { useChord } from "../useChord";
import { place } from "./placement";
import type { Placement } from "./placement";

/**
 * The hover hint: what a control does, and the key that does it without it.
 *
 * It replaces `title` on every control that has a chord. The native tooltip
 * cannot render a key cap, appears after a delay the page does not control,
 * and — the reason this exists — carries a string that has to be written by
 * hand, so it went stale the moment the keymap gained a mode. Here the chord
 * comes from the composed keymap, so it follows the platform, the browser
 * overrides, the user's `keybindings.json` and whether they are in vim mode.
 *
 * The child is cloned rather than wrapped. A wrapper element would land in the
 * middle of the flex rows these buttons live in and change their spacing, and
 * this has to be droppable onto an existing button without touching layout.
 */

const OPEN_DELAY_MS = 320;

export interface KeyHintProps {
  /** What the control does, in a few words. Sentence case, no trailing stop. */
  readonly label: string;
  /** The command whose live chord to show. */
  readonly command?: CommandId;
  /**
   * A literal chord, for the handful of keys no command owns — Escape closing
   * a transient surface, the digits on a switcher.
   */
  readonly chord?: string;
  readonly placement?: Placement;
  readonly children: ReactElement<HTMLAttributes<HTMLElement>>;
}

/** Each press is its own cap: `Ctrl+K Ctrl+Z` is two keys, not one long one. */
function ChordKeys({ chord }: { readonly chord: string }): ReactNode {
  return chord.split(" ").map((press, index) => (
    <kbd key={`${press}-${index}`} className="khint-key">
      {press}
    </kbd>
  ));
}

interface Anchored {
  readonly rect: DOMRect;
  /** Pointer hovers dismiss on leave; focus hints wait for blur. */
  readonly source: "pointer" | "focus";
}

export function KeyHint({ label, command, chord, placement = "bottom", children }: KeyHintProps) {
  const resolved = useChord(command) ?? chord;
  const [anchored, setAnchored] = useState<Anchored>();
  const [position, setPosition] = useState<{ top: number; left: number }>();
  const tip = useRef<HTMLDivElement>(null);
  const timer = useRef<number>();
  const id = useId();

  const close = useCallback(() => {
    window.clearTimeout(timer.current);
    setAnchored(undefined);
    setPosition(undefined);
  }, []);

  const open = useCallback((element: HTMLElement, source: Anchored["source"]) => {
    setAnchored({ rect: element.getBoundingClientRect(), source });
  }, []);

  // Measured, then placed, before paint — so it never appears in the wrong
  // spot and jumps once its own size is known.
  useLayoutEffect(() => {
    const element = tip.current;
    if (!anchored || !element) return;
    setPosition(
      place(anchored.rect, element.getBoundingClientRect(), placement, {
        width: window.innerWidth,
        height: window.innerHeight,
      }),
    );
  }, [anchored, placement]);

  // Anything that moves the control out from under the hint dismisses it: the
  // rect was measured once and there is no cheap way to keep it true.
  useEffect(() => {
    if (!anchored) return;
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
    };
  }, [anchored, close]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const props = children.props;
  const chain = <E,>(theirs: ((event: E) => void) | undefined, ours: (event: E) => void) =>
    (event: E) => {
      theirs?.(event);
      ours(event);
    };

  const trigger = cloneElement(children, {
    // A pointer waits: hints that fire instantly turn a sweep across a toolbar
    // into a strobe. The keyboard does not — arriving by Tab is already a
    // deliberate request to know what this is.
    onPointerEnter: chain(props.onPointerEnter, (event) => {
      const element = event.currentTarget as HTMLElement;
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => open(element, "pointer"), OPEN_DELAY_MS);
    }),
    onPointerLeave: chain(props.onPointerLeave, () => anchored?.source !== "focus" && close()),
    onPointerDown: chain(props.onPointerDown, close),
    onFocus: chain(props.onFocus, (event) => open(event.currentTarget as HTMLElement, "focus")),
    onBlur: chain(props.onBlur, close),
    "aria-describedby": anchored ? id : props["aria-describedby"],
    ...(resolved ? { "aria-keyshortcuts": resolved } : {}),
  });

  return (
    <>
      {trigger}
      {anchored
        ? createPortal(
            <div
              ref={tip}
              id={id}
              role="tooltip"
              className="modal-popover-surface khint"
              // Hidden until placed, rather than unmounted: it has to be in the
              // document to be measured at all.
              style={{ top: position?.top ?? 0, left: position?.left ?? 0, opacity: position ? 1 : 0 }}
            >
              <span className="khint-label">{label}</span>
              {resolved ? (
                <span className="khint-keys">
                  <ChordKeys chord={resolved} />
                </span>
              ) : null}
            </div>,
            document.body,
          )
        : null}
    </>
  );
}
