import React, { useCallback, useEffect, useRef, useState } from "react";
import { UrlBar } from "./UrlBar";
import { ScreencastSurface } from "./ScreencastSurface";
import { BrowserSetup } from "./BrowserSetup";
import { useBrowserPane } from "./useBrowserPane";

/**
 * A browser in a pane.
 *
 * The page runs as a native Chromium tab in the opman process. A live mirror
 * preserves the browser's cookies, sign-in state, and normal page behavior.
 * The agent's `browser_*` tools act on that same tab, which lets a person
 * watch an agent work and take the keyboard back mid-task.
 */

export interface BrowserPanelProps {
  readonly paneId: string;
  /** The project this browser belongs to; browsers are per project. */
  readonly project: string;
  readonly url: string | null;
  /**
   * Raised by the owner to mean "go to `url` now", which the URL alone cannot
   * say — the tab keeps its own page, so an earlier URL written back looks like
   * nothing happened. Stepping through the pane's history is the one thing that
   * raises it.
   */
  readonly reveal: number;
  readonly focused: boolean;
  /** Persist the current URL on the widget so a reload comes back to it. */
  readonly onUrlChanged: (url: string) => void;
}

export const BrowserTabPanel: React.FC<BrowserPanelProps> = React.memo(function BrowserTabPanel({
  paneId,
  project,
  url,
  reveal,
  focused,
  onUrlChanged,
}) {
  const pane = useBrowserPane(paneId, project, url, reveal, onUrlChanged);
  const surfaceRef = useRef<HTMLDivElement>(null);
  const { resize } = pane;
  // Shown in the header, so "my shortcuts stopped working" is answered on screen
  // rather than being a thing the user has to know.
  const [capturing, setCapturing] = useState(false);

  // Keep the page's viewport the shape of the pane, so media queries and sticky
  // headers behave as they would at this size in a real window.
  useEffect(() => {
    const element = surfaceRef.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      if (width > 0 && height > 0) resize(width, height);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [resize]);

  const retry = useCallback(() => pane.retry(), [pane]);

  return (
    <div className="bwp-panel">
      <UrlBar
        url={pane.url}
        loading={pane.loading}
        onGo={pane.go}
        onBack={pane.back}
        onForward={pane.forward}
        onReload={pane.reload}
        onEndSession={pane.endSession}
        capturing={capturing}
      />

      <div className="bwp-surface" ref={surfaceRef}>
        {pane.error !== null ? (
          pane.browserSetup ? (
            <BrowserSetup guide={pane.browserSetup} onRetry={retry} />
          ) : (
            <div className="bwp-error" role="alert">
              <p>{pane.error}</p>
              <button type="button" className="bwp-retry" onClick={retry}>
                Try again
              </button>
            </div>
          )
        ) : pane.loading && !pane.url ? (
          <div className="bwp-error" role="status">Opening browser…</div>
        ) : (
          <ScreencastSurface
            paneId={paneId}
            focused={focused}
            scale={pane.scale}
            onCaptureChange={setCapturing}
            onPageChanged={pane.adopt}
          />
        )}
      </div>
    </div>
  );
});

export { BrowserPanel } from "./BrowserTabs";
export { BrowserPanel as default } from "./BrowserTabs";
