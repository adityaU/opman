import { useCallback, useEffect, useRef, useState } from "react";
import {
  browserBack,
  browserClose,
  browserForward,
  browserNavigate,
  browserOpen,
  browserReload,
  browserResize,
  browserSetMode,
  type BrowserMode,
  type BrowserPage,
} from "../api/browser";
import { ApiError, type BrowserInstallGuide } from "../api/client";

/**
 * A browser pane's navigation state.
 *
 * The server's tab is the source of truth for where the pane is, not this hook:
 * an agent can navigate the same pane through MCP, and the pane has to end up
 * showing that. Every action therefore adopts the page the server reports back
 * rather than assuming the URL it asked for is the one that loaded.
 */

export interface BrowserPaneState {
  readonly url: string;
  readonly title: string;
  readonly mode: BrowserMode;
  readonly loading: boolean;
  readonly error: string | null;
  readonly browserSetup: BrowserInstallGuide | null;
}

const BLANK: BrowserPaneState = {
  url: "",
  title: "",
  mode: "screencast",
  loading: false,
  error: null,
  browserSetup: null,
};

export interface BrowserPaneControls extends BrowserPaneState {
  /** Device pixels per CSS pixel in the captured frames, as the server applied it. */
  readonly scale: number;
  readonly go: (url: string) => void;
  readonly back: () => void;
  readonly forward: () => void;
  readonly reload: () => void;
  readonly retry: () => void;
  readonly toggleMode: () => void;
  readonly resize: (width: number, height: number) => void;
  /** End the browser session — closes the tab the project has been sharing. */
  readonly endSession: () => void;
}

export function useBrowserPane(
  paneId: string,
  project: string,
  initialUrl: string | null,
  reveal: number,
  onUrlChanged: (url: string) => void,
): BrowserPaneControls {
  const [state, setState] = useState<BrowserPaneState>(BLANK);
  const [scale, setScale] = useState(1);
  // Kept in a ref as well so the resize observer can read it without becoming a
  // dependency that re-subscribes on every navigation.
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const settle = useCallback(
    (page: BrowserPage) => {
      if (!alive.current) return;
      setState((previous) => ({
        url: page.url || previous.url,
        title: page.title || previous.title,
        // `mode` only comes back from calls that could have changed it; a back
        // or reload leaves the pane rendering the way it already was.
        mode: page.mode ?? previous.mode,
        loading: false,
        error: null,
        browserSetup: null,
      }));
      if (page.url) onUrlChanged(page.url);
    },
    [onUrlChanged],
  );

  const run = useCallback(
    (action: () => Promise<BrowserPage>) => {
      setState((previous) => ({ ...previous, loading: true, error: null, browserSetup: null }));
      action()
        .then(settle)
        .catch((error: unknown) => {
          if (!alive.current) return;
          setState((previous) => ({
            ...previous,
            loading: false,
            error: error instanceof Error ? error.message : String(error),
            browserSetup: error instanceof ApiError ? (error.browserSetup ?? null) : null,
          }));
        });
    },
    [settle],
  );

  // Connect once. The server sends the pane to `initialUrl` only if it had to
  // create the tab; a tab that is already running keeps its page, which is what
  // makes reopening a browser pick up an agent's work in progress.
  useEffect(() => {
    let cancelled = false;
    setState((previous) => ({ ...previous, loading: true, error: null, browserSetup: null }));
    browserOpen(paneId, project, initialUrl ?? undefined)
      .then((page) => {
        if (!cancelled) settle(page);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setState((previous) => ({
          ...previous,
          loading: false,
          error: error instanceof Error ? error.message : String(error),
          browserSetup: error instanceof ApiError ? (error.browserSetup ?? null) : null,
        }));
      });
    return () => {
      cancelled = true;
    };
    // Deliberately keyed on the pane alone: `initialUrl` changes as the user
    // browses, and re-running on it would drag the tab back to where it started.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [paneId, project]);

  // Unmounting does NOT close the tab. A browser is per project and outlives the
  // pane showing it: an agent may still be driving it, and closing on unmount
  // would throw away that work the moment the user switched windows. The tab
  // goes when the server does, or when the user closes it explicitly.

  const go = useCallback(
    (url: string) => {
      const trimmed = url.trim();
      if (trimmed) run(() => browserNavigate(paneId, project, trimmed));
    },
    [paneId, project, run],
  );

  /**
   * Go where the owner says, when it says so explicitly.
   *
   * The connect effect above ignores `initialUrl` changes on purpose, so that
   * browsing does not drag the tab back to where it started. But stepping
   * through the *pane's* history is exactly a request to go back to an earlier
   * URL, and it is indistinguishable from ordinary browsing by the URL alone.
   * The counter is what distinguishes it: it only ever rises, and only a history
   * move raises it.
   */
  const revealed = useRef(reveal);
  useEffect(() => {
    if (reveal <= revealed.current) return;
    revealed.current = reveal;
    const target = initialUrl?.trim();
    if (target) run(() => browserNavigate(paneId, project, target));
    // `initialUrl` is read, not depended on: it changes as the user browses, and
    // this must fire on the counter alone.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reveal]);

  const back = useCallback(() => run(() => browserBack(paneId)), [paneId, run]);
  const forward = useCallback(() => run(() => browserForward(paneId)), [paneId, run]);
  const reload = useCallback(() => run(() => browserReload(paneId)), [paneId, run]);
  const retry = useCallback(
    () => run(() => browserOpen(paneId, project, initialUrl ?? undefined)),
    [initialUrl, paneId, project, run],
  );

  /**
   * The manual override for a site that passes the framing check but still
   * breaks inside an iframe — a header cannot express "renders wrong", so the
   * user gets the switch.
   */
  const toggleMode = useCallback(() => {
    setState((previous) => {
      const next: BrowserMode = previous.mode === "iframe" ? "screencast" : "iframe";
      void browserSetMode(paneId, next).catch(() => {});
      return { ...previous, mode: next };
    });
  }, [paneId]);

  /**
   * Follow the pane's box, at this display's pixel density. The applied scale
   * comes back because the server may have refused the full ratio on a wide
   * pane, and a click is placed against what was actually captured.
   */
  const resize = useCallback(
    (width: number, height: number) => {
      const ratio = window.devicePixelRatio || 1;
      browserResize(paneId, Math.round(width), Math.round(height), ratio)
        .then((applied) => {
          if (alive.current) setScale(applied.scale || 1);
        })
        .catch(() => {});
    },
    [paneId],
  );

  /**
   * Discard the tab and start clean. The counterpart to not closing on unmount:
   * a session that survives the pane needs one deliberate way to end, or a
   * logged-in page lingers with nothing showing it.
   */
  const endSession = useCallback(() => {
    setState(BLANK);
    browserClose(paneId)
      .then(() => browserOpen(paneId, project))
      .then(settle)
      .catch(() => {});
  }, [paneId, project, settle]);

  return { ...state, scale, go, back, forward, reload, retry, toggleMode, resize, endSession };
}
