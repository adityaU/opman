import React, { useCallback, useEffect, useRef } from "react";
import { UrlBar } from "./UrlBar";
import { ScreencastSurface } from "./ScreencastSurface";
import { useBrowserPane } from "./useBrowserPane";

/**
 * A browser in a pane.
 *
 * The page runs as a headless Chromium tab in the opman process, not in this
 * document. What changes between the two render modes is only how the pane
 * *shows* it: an iframe when the site permits framing (cheap, crisp, native
 * scrolling) and a live mirror of the real tab when it does not. Either way the
 * agent's `browser_*` tools act on that same tab, which is what lets a person
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

export const BrowserPanel: React.FC<BrowserPanelProps> = React.memo(function BrowserPanel({
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

  // Keep the headless viewport the shape of the pane, so media queries and
  // sticky headers behave as they would at this size in a real window.
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

  const retry = useCallback(() => pane.reload(), [pane]);

  return (
    <div className="bwp-panel">
      <UrlBar
        url={pane.url}
        loading={pane.loading}
        mode={pane.mode}
        onGo={pane.go}
        onBack={pane.back}
        onForward={pane.forward}
        onReload={pane.reload}
        onToggleMode={pane.toggleMode}
        onEndSession={pane.endSession}
      />

      <div className="bwp-surface" ref={surfaceRef}>
        {pane.error !== null ? (
          <div className="bwp-error" role="alert">
            <p>{pane.error}</p>
            <button type="button" className="bwp-retry" onClick={retry}>
              Try again
            </button>
          </div>
        ) : pane.mode === "iframe" && pane.url ? (
          <iframe
            className="bwp-iframe"
            src={pane.url}
            title="Browser pane"
            // The page is a stranger: give it no access to this document and no
            // ambient permissions it did not ask for.
            sandbox="allow-scripts allow-forms allow-popups allow-same-origin"
            referrerPolicy="no-referrer"
          />
        ) : (
          <ScreencastSurface paneId={paneId} focused={focused} />
        )}
      </div>
    </div>
  );
});

export default BrowserPanel;
