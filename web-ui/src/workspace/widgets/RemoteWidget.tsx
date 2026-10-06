import React, { useEffect, useRef, useState } from "react";
import { embedUrl, widgetKey } from "../../embed/embedUrl";
import { isSameOrigin, readFromEmbed } from "../../embed/messages";
import { withServer } from "../foreignOpen";
import type { PaneId, WidgetState } from "../types";

/**
 * A widget from another opman server: that server's own instance, embedded.
 *
 * The frame's URL is fixed at mount. After that the two sides keep each other
 * told by message — the parent pointing the pane somewhere new (`set-widget`),
 * the embed reporting where it went (`widget-changed`) — so a session created
 * inside, or a file opened inside, never costs a reload of the frame. The last
 * widget either side named is remembered so neither echoes the other.
 */

export interface RemoteWidgetProps {
  readonly widget: WidgetState;
  readonly server: string;
  readonly paneId: PaneId;
  readonly focused: boolean;
  /** The embed moved on; record it on the pane so a reload comes back to it. */
  readonly onChanged: (pane: PaneId, widget: WidgetState) => void;
}

export const RemoteWidget: React.FC<RemoteWidgetProps> = function RemoteWidget({
  widget,
  server,
  paneId,
  focused,
  onChanged,
}) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [src] = useState(() => embedUrl(server, widget));
  const known = useRef(widgetKey(widget));
  const latest = useRef({ widget, onChanged });
  latest.current = { widget, onChanged };

  const send = (target: WidgetState) =>
    frame.current?.contentWindow?.postMessage(
      { type: "opman:set-widget", widget: target },
      location.origin,
    );

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      if (event.source !== frame.current?.contentWindow || !isSameOrigin(event)) return;
      const message = readFromEmbed(event.data);
      if (!message || message.server !== server) return;
      // A target set before the frame had loaded was posted to nobody; ready
      // is the first moment the embed can hear it.
      if (message.type === "opman:embed-ready") send(latest.current.widget);
      if (message.type !== "opman:widget-changed") return;
      known.current = widgetKey(message.widget);
      latest.current.onChanged(paneId, withServer(message.widget, server));
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [paneId, server]);

  useEffect(() => {
    const key = widgetKey(widget);
    if (key === known.current) return;
    known.current = key;
    send(widget);
  }, [widget]);

  // The pane is a focus scope; inside it, the caret belongs to the embed.
  useEffect(() => {
    if (focused) frame.current?.focus({ preventScroll: true });
  }, [focused]);

  return (
    <div className="wsp-remote">
      <iframe
        ref={frame}
        className="wsp-remote-frame"
        src={src}
        title={`${widget.kind} in ${widget.projectPath} on ${server}`}
        // Clipboard for copy buttons and the terminal; same origin already.
        allow="clipboard-read; clipboard-write; fullscreen"
      />
    </div>
  );
};
