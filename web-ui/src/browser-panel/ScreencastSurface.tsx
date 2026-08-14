import React, { useCallback, useEffect, useRef, useState } from "react";
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

/** Chromium encodes at most this wide; frames scale to fit the pane. */
const FRAME_WIDTH = 1280;

interface ScreencastSurfaceProps {
  readonly paneId: string;
  readonly focused: boolean;
}

export const ScreencastSurface: React.FC<ScreencastSurfaceProps> = React.memo(
  function ScreencastSurface({ paneId, focused }) {
    const [frame, setFrame] = useState<string | null>(null);
    const surfaceRef = useRef<HTMLDivElement>(null);
    const imageRef = useRef<HTMLImageElement>(null);

    useEffect(() => {
      const source = createBrowserFrameSSE(paneId);
      source.addEventListener("frame", (event) => {
        setFrame((event as MessageEvent<string>).data);
      });
      return () => source.close();
    }, [paneId]);

    /**
     * Element coordinates → page coordinates. The image is letterboxed to
     * preserve aspect, so the scale factor comes from its rendered width rather
     * than the container's.
     */
    const toPage = useCallback((event: React.MouseEvent): { x: number; y: number } | null => {
      const image = imageRef.current;
      if (!image || image.naturalWidth === 0) return null;
      const box = image.getBoundingClientRect();
      const scale = image.naturalWidth / box.width;
      return {
        x: Math.round((event.clientX - box.left) * scale),
        y: Math.round((event.clientY - box.top) * scale),
      };
    }, []);

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
        // Let the app keep its own chords: the pane is a surface inside a
        // workspace, not a window that owns the keyboard.
        if (event.key === "Escape" && !event.ctrlKey && !event.metaKey) return;
        event.preventDefault();
        event.stopPropagation();

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

    return (
      <div
        ref={surfaceRef}
        className="bwp-screencast"
        // Focusable so the pane can receive keys, but out of the tab ring: a
        // remote page should not be a stop on the way through the app's own UI.
        tabIndex={-1}
        role="application"
        aria-label="Browser page"
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
          <img
            ref={imageRef}
            className="bwp-frame"
            src={`data:image/jpeg;base64,${frame}`}
            width={FRAME_WIDTH}
            alt=""
            draggable={false}
          />
        )}
      </div>
    );
  },
);
