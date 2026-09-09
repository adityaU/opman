import React, { useCallback, useEffect, useRef, useState } from "react";
import { BROWSER_CAPTURE_ATTRIBUTE, releasesKeyboard } from "./capture";
import {
  browserKey,
  browserMouse,
  browserScroll,
  browserTextInput,
  createBrowserFrameSSE,
  type BrowserMouseKind,
} from "../api/browser";

/**
 * The page as live JPEG frames, for sites that refuse to be framed.
 *
 * Input is forwarded rather than simulated: a pointer event is translated from
 * the element's box into page coordinates and dispatched as a real CDP mouse
 * event, so hover, drag and focus behave the way they would in a window. The
 * page never runs in this document, which is also why a hostile site cannot
 * reach the app around it.
 */

interface ScreencastSurfaceProps {
  readonly paneId: string;
  readonly focused: boolean;
  /**
   * Device pixels per CSS pixel in the frames, as the server applied it. Frames
   * are captured at the display's density so they land pixel-exact, which means
   * their intrinsic size is no longer the page's coordinate space.
   */
  readonly scale: number;
  /** Raised while the page holds the keyboard, so the header can say so. */
  readonly onCaptureChange?: (capturing: boolean) => void;
}

export const ScreencastSurface: React.FC<ScreencastSurfaceProps> = React.memo(
  function ScreencastSurface({ paneId, focused, scale, onCaptureChange }) {
    const [frame, setFrame] = useState<string | null>(null);
    const surfaceRef = useRef<HTMLDivElement>(null);
    const imageRef = useRef<HTMLImageElement>(null);
    /** When Escape was last pressed, for the double-tap that releases focus. */
    const lastEscape = useRef<number | undefined>(undefined);

    useEffect(() => {
      const source = createBrowserFrameSSE(paneId);
      source.addEventListener("frame", (event) => {
        setFrame((event as MessageEvent<string>).data);
      });
      return () => source.close();
    }, [paneId]);

    /**
     * Element coordinates → page coordinates.
     *
     * Two conversions in one: the image is letterboxed, so its rendered width
     * rather than the container's sets the ratio — and the frame is captured in
     * device pixels while CDP places clicks in CSS pixels, so the capture scale
     * has to come back out of the intrinsic width.
     */
    const toPage = useCallback(
      (event: React.MouseEvent): { x: number; y: number } | null => {
        const image = imageRef.current;
        if (!image || image.naturalWidth === 0) return null;
        const box = image.getBoundingClientRect();
        if (box.width === 0) return null;
        const pageWidth = image.naturalWidth / (scale || 1);
        const factor = pageWidth / box.width;
        return {
          x: Math.round((event.clientX - box.left) * factor),
          y: Math.round((event.clientY - box.top) * factor),
        };
      },
      [scale],
    );

    const send = useCallback(
      (kind: BrowserMouseKind) => (event: React.MouseEvent) => {
        const point = toPage(event);
        if (!point) return;
        void browserMouse(paneId, kind, point.x, point.y).catch(() => {});
      },
      [paneId, toPage],
    );

    const onWheel = useCallback(
      (event: React.WheelEvent) => {
        const point = toPage(event as unknown as React.MouseEvent);
        void browserScroll(paneId, Math.round(event.deltaY), point?.x ?? 0, point?.y ?? 0).catch(
          () => {},
        );
      },
      [paneId, toPage],
    );

    /**
     * Printable characters go through `insertText`; everything else is a named
     * chord. Splitting them here is what keeps an accented character or an IME
     * commit from arriving as a keycode the page cannot interpret.
     */
    const onKeyDown = useCallback(
      (event: React.KeyboardEvent) => {
        event.preventDefault();
        event.stopPropagation();

        // Double Escape hands the keyboard back to the app. The first one still
        // reaches the page, because that is what closes a site's own dialog.
        if (releasesKeyboard(event.key, lastEscape.current, event.timeStamp)) {
          lastEscape.current = undefined;
          surfaceRef.current?.blur();
          return;
        }
        lastEscape.current = event.key === "Escape" ? event.timeStamp : undefined;

        const printable = event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey;
        if (printable) {
          void browserTextInput(paneId, event.key).catch(() => {});
          return;
        }
        const modifiers = [
          event.ctrlKey ? "Control" : null,
          event.altKey ? "Alt" : null,
          event.shiftKey && event.key.length > 1 ? "Shift" : null,
          event.metaKey ? "Meta" : null,
        ].filter(Boolean);
        void browserKey(paneId, [...modifiers, event.key].join("+")).catch(() => {});
      },
      [paneId],
    );

    useEffect(() => {
      if (focused) surfaceRef.current?.focus({ preventScroll: true });
    }, [focused]);

    // Focus is reported up only so the header can say the page has the keyboard;
    // dispatch does not depend on it. See `browserOwnsKey`.
    const capture = useCallback(
      (active: boolean) => () => onCaptureChange?.(active),
      [onCaptureChange],
    );

    return (
      <div
        ref={surfaceRef}
        className="bwp-screencast"
        // Focusable so the pane can receive keys, but out of the tab ring: a
        // remote page should not be a stop on the way through the app's own UI.
        tabIndex={-1}
        role="application"
        aria-label="Browser page"
        // Marks this element as owning the keyboard, which is what makes the
        // app keymap stand down before a page ever misses a keystroke.
        {...{ [BROWSER_CAPTURE_ATTRIBUTE]: "" }}
        onFocus={capture(true)}
        onBlur={capture(false)}
        onMouseDown={send("down")}
        onMouseUp={send("up")}
        onMouseMove={send("move")}
        onWheel={onWheel}
        onKeyDown={onKeyDown}
      >
        {frame === null ? (
          <div className="bwp-placeholder" aria-busy="true">
            Waiting for the page…
          </div>
        ) : (
          // No width attribute: the frame's intrinsic size is device pixels, and
          // letting CSS clamp it to the pane paints those one to one on screen.
          <img
            ref={imageRef}
            className="bwp-frame"
            src={`data:image/jpeg;base64,${frame}`}
            alt=""
            draggable={false}
          />
        )}
      </div>
    );
  },
);
