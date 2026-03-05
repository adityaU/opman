import React, { useRef, useEffect, useCallback, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import { spawnPty, ptyWrite, ptyResize, ptyKill, createPtySSE } from "./api";
import { X, Maximize2, Minimize2 } from "lucide-react";

interface Props {
  sessionId: string | null;
  onClose: () => void;
}

function uuid(): string {
  return (
    crypto.randomUUID?.() ??
    `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
  );
}

export function TerminalPanel({ sessionId, onClose }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const sseRef = useRef<EventSource | null>(null);
  const ptyIdRef = useRef<string>("");
  const [status, setStatus] = useState<"connecting" | "ready" | "error">(
    "connecting"
  );
  const [expanded, setExpanded] = useState(false);

  const handleData = useCallback((data: string) => {
    if (!ptyIdRef.current) return;
    const encoded = btoa(
      Array.from(new TextEncoder().encode(data), (b) =>
        String.fromCharCode(b)
      ).join("")
    );
    ptyWrite(ptyIdRef.current, encoded).catch(() => {});
  }, []);

  const handleResize = useCallback(
    (dims: { cols: number; rows: number }) => {
      if (!ptyIdRef.current) return;
      ptyResize(ptyIdRef.current, dims.rows, dims.cols).catch(() => {});
    },
    []
  );

  useEffect(() => {
    if (!containerRef.current) return;

    const id = uuid();
    ptyIdRef.current = id;

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: "block",
      fontFamily:
        "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
      fontSize: 13,
      lineHeight: 1.2,
      theme: {
        background: "#0a0a0a",
        foreground: "#eeeeee",
        cursor: "#eeeeee",
        selectionBackground: "#33467c",
      },
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

    const rows = term.rows;
    const cols = term.cols;

    spawnPty("shell", id, rows, cols)
      .then(() => {
        setStatus("ready");

        const sse = createPtySSE(id);
        sseRef.current = sse;

        sse.addEventListener("output", (event: MessageEvent) => {
          try {
            const raw = atob(event.data);
            const bytes = new Uint8Array(raw.length);
            for (let i = 0; i < raw.length; i++)
              bytes[i] = raw.charCodeAt(i);
            term.write(bytes);
          } catch {
            term.write(event.data);
          }
        });
      })
      .catch((err) => {
        console.error("Failed to spawn terminal PTY:", err);
        setStatus("error");
        term.write(
          "\r\n\x1b[31mFailed to spawn terminal process.\x1b[0m\r\n"
        );
      });

    const observer = new ResizeObserver(() => {
      if (fitRef.current) fitRef.current.fit();
    });
    observer.observe(containerRef.current);

    return () => {
      observer.disconnect();
      if (sseRef.current) {
        sseRef.current.close();
        sseRef.current = null;
      }
      if (ptyIdRef.current) {
        ptyKill(ptyIdRef.current).catch(() => {});
      }
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
      ptyIdRef.current = "";
    };
  }, [handleData, handleResize]);

  return (
    <div className={`terminal-panel ${expanded ? "expanded" : ""}`}>
      <div className="terminal-panel-header">
        <span className="terminal-panel-title">Terminal</span>
        <div className="terminal-panel-actions">
          <button onClick={() => setExpanded((v) => !v)} title="Toggle size">
            {expanded ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
          </button>
          <button onClick={onClose} title="Close terminal">
            <X size={14} />
          </button>
        </div>
      </div>
      <div ref={containerRef} className="terminal-panel-body">
        {status === "connecting" && (
          <div className="terminal-overlay">Spawning terminal...</div>
        )}
        {status === "error" && (
          <div className="terminal-overlay error">
            Failed to start terminal
          </div>
        )}
      </div>
    </div>
  );
}
