import { useEffect, useRef } from "react";
import { widgetKey } from "../embed/embedUrl";
import { isSameOrigin, readFromEmbed } from "../embed/messages";
import { useKeymapContext } from "../keybindings/KeymapContext";
import { withServer } from "./foreignOpen";
import type { WidgetState } from "./types";

/**
 * The parent's half: requests from embedded panes to open something, and
 * workspace chords pressed while the caret was inside one.
 *
 * Every embed of one server hears that server's MCP events, so three panes from
 * the dev box would each forward the same "open this file". The repeats are
 * dropped — the same target from the same server inside a short window is one
 * request.
 */

const REPEAT_WINDOW_MS = 1500;

export function useEmbedRequests(
  enabled: boolean,
  openForeignWidget: (widget: WidgetState) => void,
): void {
  const { runCommand } = useKeymapContext();
  const latest = useRef({ openForeignWidget, runCommand });
  latest.current = { openForeignWidget, runCommand };
  const recent = useRef(new Map<string, number>());

  useEffect(() => {
    if (!enabled) return;
    const onMessage = (event: MessageEvent) => {
      if (!isSameOrigin(event) || event.source === window) return;
      const message = readFromEmbed(event.data);
      if (!message) return;

      if (message.type === "opman:run-command") {
        latest.current.runCommand(message.command);
        return;
      }
      if (message.type !== "opman:open-widget") return;

      const key = `${message.server}\u0000${requestKey(message.widget)}`;
      const now = Date.now();
      if (now - (recent.current.get(key) ?? 0) < REPEAT_WINDOW_MS) return;
      recent.current.set(key, now);
      latest.current.openForeignWidget(withServer(message.widget, message.server));
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [enabled]);
}

/** What a request asks for, without the counters each embed mints for itself. */
export function requestKey(widget: WidgetState): string {
  if (widget.kind === "files") {
    return `files\u0000${widget.projectPath}\u0000${widget.open?.path ?? ""}\u0000${widget.open?.line ?? ""}`;
  }
  if (widget.kind === "browser") return `browser\u0000${widget.projectPath}\u0000${widget.url ?? ""}`;
  return widgetKey(widget);
}
