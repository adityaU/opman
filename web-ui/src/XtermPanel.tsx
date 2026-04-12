import React, { useRef, useEffect, useCallback, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import { spawnPty, ptyWrite, ptyResize, ptyKill, createPtySSE } from "./api";

interface Props {
  /** PTY type to spawn: "shell", "neovim", "git", or "opencode" */
  kind: "neovim" | "shell" | "git" | "opencode";
  /** Whether this panel currently has focus */
  focused?: boolean;
  /** Optional session ID (only used for "opencode" kind) */
  sessionId?: string;
}

/** Generate a random UUID v4 */
function uuid(): string {
  return crypto.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

function getTerminalTheme() {
  const css = getComputedStyle(document.documentElement);
  const text = css.getPropertyValue("--color-text").trim() || "var(--color-text)";
  const muted = css.getPropertyValue("--color-text-muted").trim() || "var(--color-text-muted)";
  const primary = css.getPropertyValue("--color-primary").trim() || "var(--color-primary)";
  const secondary = css.getPropertyValue("--color-secondary").trim() || "var(--color-secondary)";
  const accent = css.getPropertyValue("--color-accent").trim() || "var(--color-accent)";
  const success = css.getPropertyValue("--color-success").trim() || "var(--color-success)";
  const warning = css.getPropertyValue("--color-warning").trim() || "var(--color-warning)";
  const error = css.getPropertyValue("--color-error").trim() || "var(--color-error)";
  const info = css.getPropertyValue("--color-info").trim() || "var(--color-info)";
  const panel = css.getPropertyValue("--color-bg-panel").trim() || "var(--color-bg-panel)";
  return {
    background: "transparent",
    foreground: text,
    cursor: text,
    selectionBackground: `color-mix(in srgb, ${secondary} 28%, transparent)`,
    black: panel,
    red: error,
    green: success,
    yellow: warning,
    blue: secondary,
    magenta: accent,
    cyan: info,
    white: text,
    brightBlack: muted,
    brightRed: error,
    brightGreen: success,
    brightYellow: warning,
    brightBlue: secondary,
    brightMagenta: accent,
    brightCyan: info,
    brightWhite: primary,
    selectionForeground: text,
  };
}

export function XtermPanel({ kind, focused, sessionId }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const sseRef = useRef<EventSource | null>(null);
  const ptyIdRef = useRef<string>("");
  const [status, setStatus] = useState<"connecting" | "ready" | "error">("connecting");

  // Send keystrokes to backend PTY
  const handleData = useCallback((data: string) => {
    if (!ptyIdRef.current) return;
    // Encode as base64 for safe transport of binary/control chars
    const encoded = btoa(
      Array.from(new TextEncoder().encode(data), (b) => String.fromCharCode(b)).join("")
    );
    ptyWrite(ptyIdRef.current, encoded).catch(() => {});
  }, []);

  // Handle resize
  const handleResize = useCallback((dims: { cols: number; rows: number }) => {
    if (!ptyIdRef.current) return;
    ptyResize(ptyIdRef.current, dims.rows, dims.cols).catch(() => {});
  }, []);

  useEffect(() => {
    if (!containerRef.current) return;

    const id = uuid();
    ptyIdRef.current = id;

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: "block",
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
      fontSize: 13,
      lineHeight: 1.2,
      allowTransparency: true,
      theme: getTerminalTheme(),
      allowProposedApi: true,
    });

    const fitAddon = new FitAddon();
    const webLinksAddon = new WebLinksAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(webLinksAddon);

    term.open(containerRef.current);
    fitAddon.fit();

    term.onData(handleData);
    term.onResize(handleResize);

    termRef.current = term;
    fitRef.current = fitAddon;

    // Spawn the PTY process on the backend
    const rows = term.rows;
    const cols = term.cols;

    spawnPty(kind, id, rows, cols, sessionId)
      .then(() => {
        setStatus("ready");

        // Connect SSE for raw PTY output
        const sse = createPtySSE(id);
        sseRef.current = sse;

        sse.addEventListener("output", (event: MessageEvent) => {
          try {
            // Data comes as base64-encoded raw VT100 bytes
            const raw = atob(event.data);
            const bytes = new Uint8Array(raw.length);
            for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
            term.write(bytes);
          } catch {
            // Fallback: treat as plain text
            term.write(event.data);
          }
        });

        sse.onerror = () => {
          // EventSource auto-reconnects
        };
      })
      .catch((err) => {
        console.error(`Failed to spawn ${kind} PTY:`, err);
        setStatus("error");
        term.write(`\r\n\x1b[31mFailed to spawn ${kind} process.\x1b[0m\r\n`);
      });

    // Fit on container resize
    const observer = new ResizeObserver(() => {
      if (fitRef.current) {
        fitRef.current.fit();
      }
    });
    observer.observe(containerRef.current);

    // Cleanup: kill PTY and close SSE on unmount
    return () => {
      observer.disconnect();
      if (sseRef.current) {
        sseRef.current.close();
        sseRef.current = null;
      }
      // Fire-and-forget kill
      if (ptyIdRef.current) {
        ptyKill(ptyIdRef.current).catch(() => {});
      }
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
      ptyIdRef.current = "";
    };
  }, [kind, sessionId, handleData, handleResize]);

  // Focus terminal when panel becomes focused
  useEffect(() => {
    if (focused && termRef.current) {
      termRef.current.focus();
    }
  }, [focused]);

  return (
    <div ref={containerRef} className="terminal-container">
      {status === "connecting" && (
        <div className="terminal-overlay">Spawning {kind}...</div>
      )}
      {status === "error" && (
        <div className="terminal-overlay error">Failed to start {kind}</div>
      )}
    </div>
  );
}
