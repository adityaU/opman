import React, { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { SearchAddon } from "@xterm/addon-search";
import { WebglAddon } from "@xterm/addon-webgl";
import { ptyResize, spawnPty } from "../api";
import { connectPty } from "./ptyConnection";
import { shellExists } from "./useShells";
import { getTerminalTheme, TERM_OPTIONS, type PtyKind, type ShellStatus, type TerminalRuntime } from "./types";
import { THEME_CHANGED_EVENT } from "../utils/theme";
import { installFeatures, type FeatureHooks } from "./features";
import { loadFontSize } from "./fontSize";

/**
 * Show one shell in one container.
 *
 * The shell is not this view's to end. Unmounting — a pane closing, a zoom, a
 * window switch, a reload — tears down the xterm and the stream and leaves the
 * program running, which is what makes a build survive rearranging the
 * workspace around it. Only an explicit kill stops a shell.
 *
 * Coming back to a running shell repaints from the server's retained
 * scrollback, so re-attaching looks like the terminal was never away.
 */

export interface TerminalSession {
  readonly status: ShellStatus;
  /** The xterm and its addons, for search and the mobile key bar. */
  readonly runtimeRef: React.MutableRefObject<TerminalRuntime | null>;
}

export function useTerminalSession(
  ptyId: string | null,
  kind: PtyKind,
  projectPath: string | null,
  sessionId: string | null,
  containerRef: React.RefObject<HTMLDivElement | null>,
  visible: boolean,
  /** Optional rewrite applied to every keystroke before it reaches the pty —
   *  how the touch key bar's sticky Ctrl/Alt reach the soft keyboard's output.
   *  Held in a ref so arming a modifier never re-installs the data handler. */
  transformRef?: React.MutableRefObject<((data: string) => string) | null>,
  /** What the panel shows for the terminal's own events: menu, notices, copy mode. */
  hooks?: FeatureHooks,
): TerminalSession {
  const [status, setStatus] = useState<ShellStatus>("connecting");
  const runtimeRef = useRef<TerminalRuntime | null>(null);

  // The session id only matters to the kinds that attach to a conversation, and
  // reading it through a ref keeps a chat switch from rebuilding the terminal.
  const attachTo = useRef(sessionId);
  attachTo.current = sessionId;
  const hooksRef = useRef(hooks);
  hooksRef.current = hooks;

  useEffect(() => {
    const container = containerRef.current;
    if (!ptyId || !container) return;

    let live = true;
    setStatus("connecting");

    const term = new Terminal({
      ...TERM_OPTIONS,
      fontSize: loadFontSize(),
      theme: getTerminalTheme(),
    });
    const fit = new FitAddon();
    const search = new SearchAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon());
    term.loadAddon(search);
    term.open(container);
    loadGpuRenderer(term);
    fit.fit();
    const features = installFeatures(term, container, fit, {
      onMenu: (x, y) => hooksRef.current?.onMenu(x, y),
      onNotice: (text) => hooksRef.current?.onNotice(text),
      onCopyMode: (active) => hooksRef.current?.onCopyMode(active),
    });

    // Keys typed before the shell is reached wait here, in order.
    let early = "";
    term.onData((data) => {
      const transform = transformRef?.current;
      const keys = transform ? transform(data) : data;
      if (runtime.connection) runtime.connection.send(keys);
      else early += keys;
    });
    term.onResize(({ rows, cols }) => {
      void ptyResize(ptyId, rows, cols).catch(() => {});
    });

    // Never fit to an empty box: a pane in a background workspace window has
    // its contents skipped and measures 0x0, and fitting to that resizes the
    // PTY on the server to one column and back on the way in.
    const observer = new ResizeObserver(() => {
      if (container.clientWidth === 0 || container.clientHeight === 0) return;
      fit.fit();
    });
    observer.observe(container);

    const runtime: TerminalRuntime = {
      term,
      fit,
      search,
      connection: null,
      observer,
      actions: features.actions,
    };
    runtimeRef.current = runtime;

    attach(ptyId, kind, term.rows, term.cols, projectPath, attachTo.current)
      .then((replay) => {
        if (!live) return;
        setStatus("ready");
        runtime.connection = connectPty(ptyId, replay, (bytes) => term.write(bytes));
        if (early) runtime.connection.send(early);
        early = "";
      })
      .catch(() => {
        if (!live) return;
        setStatus("error");
        term.write("\r\n\x1b[31mFailed to open this terminal.\x1b[0m\r\n");
      });

    return () => {
      live = false;
      // Detach only. The program on the other end keeps running.
      observer.disconnect();
      features.dispose();
      runtime.connection?.close();
      term.dispose();
      runtimeRef.current = null;
    };
    // `sessionId` is read through a ref on purpose — see `attachTo` above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ptyId, kind, projectPath, containerRef, transformRef]);

  // xterm copies the palette at construction, so a terminal opened before a
  // theme switch would keep the old one until it was reopened.
  useEffect(() => {
    const repaint = () => {
      const runtime = runtimeRef.current;
      if (runtime) runtime.term.options.theme = getTerminalTheme();
    };
    window.addEventListener(THEME_CHANGED_EVENT, repaint);
    return () => window.removeEventListener(THEME_CHANGED_EVENT, repaint);
  }, []);

  // Re-fit when the panel is revealed or resized by a layout change.
  useEffect(() => {
    if (!visible) return;
    const runtime = runtimeRef.current;
    if (!runtime) return;
    const frame = requestAnimationFrame(() => runtime.fit.fit());
    return () => cancelAnimationFrame(frame);
  }, [visible, status]);

  return { status, runtimeRef };
}

/**
 * Reach the shell, and say whether it was already running.
 *
 * Only an already-running shell may replay: replaying into a freshly started
 * one would repaint history it never had. Spawn is safe to call for a live id —
 * the server hands back the running PTY — but asking first is what tells the
 * caller which of the two happened.
 */
async function attach(
  id: string,
  kind: PtyKind,
  rows: number,
  cols: number,
  project: string | null,
  sessionId: string | null,
): Promise<boolean> {
  if (await shellExists(id)) return true;
  await spawnPty(kind, id, rows, cols, {
    project,
    sessionId: sessionId ?? undefined,
  });
  return false;
}

/**
 * Draw with WebGL when the browser offers it: the DOM renderer re-lays-out every cell it
 * touches, which is what made full-screen programs (nvim, htop) and long builds crawl.
 * A lost GPU context drops back to the DOM renderer instead of leaving a blank pane.
 */
function loadGpuRenderer(term: Terminal): void {
  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => webgl.dispose());
    term.loadAddon(webgl);
  } catch {
    /* no WebGL here: the DOM renderer stays */
  }
}
