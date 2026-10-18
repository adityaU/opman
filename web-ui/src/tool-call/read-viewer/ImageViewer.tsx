import React, { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { Loading, Unavailable } from "./Unavailable";
import { useFocusTrap } from "../../hooks/useFocusTrap";

interface ImageViewerProps {
  readonly url: string;
  readonly name: string;
  readonly onMeta: (text: string) => void;
}

/** An image that fits the card; a click opens it large in a dialog. */
export function ImageViewer({ url, name, onMeta }: ImageViewerProps) {
  const [phase, setPhase] = useState<"loading" | "ready" | "failed">("loading");
  const [large, setLarge] = useState(false);

  useEffect(() => setPhase("loading"), [url]);

  return (
    <div className="rv-image-wrap">
      {phase === "loading" && <Loading />}
      {phase === "failed" ? (
        <Unavailable />
      ) : (
        <img
          className={`rv-image${phase === "ready" ? "" : " rv-hidden"}`}
          src={url}
          alt={name}
          role="button"
          tabIndex={phase === "ready" ? 0 : -1}
          aria-label={`Enlarge ${name}`}
          loading="lazy"
          decoding="async"
          onClick={() => setLarge(true)}
          onKeyDown={(event) => {
            if (event.key !== "Enter" && event.key !== " ") return;
            event.preventDefault();
            setLarge(true);
          }}
          onLoad={(e) => {
            setPhase("ready");
            const { naturalWidth: w, naturalHeight: h } = e.currentTarget;
            if (w > 0 && h > 0) onMeta(`${w} × ${h}`);
          }}
          onError={() => setPhase("failed")}
        />
      )}
      {large && <ImageDialog url={url} name={name} onClose={() => setLarge(false)} />}
    </div>
  );
}

function ImageDialog({ url, name, onClose }: { url: string; name: string; onClose: () => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  useFocusTrap(dialog);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onClose();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  return createPortal(
    <div className="rv-lightbox-backdrop modal-backdrop" role="presentation" onClick={onClose}>
      <div ref={dialog} className="rv-lightbox modal-dialog-surface" role="dialog" aria-modal="true" aria-label={name}
        onClick={(e) => e.stopPropagation()}>
        <button type="button" className="rv-lightbox-close" onClick={onClose} aria-label="Close">
          <X size={15} />
        </button>
        <img src={url} alt={name} />
      </div>
    </div>,
    document.body,
  );
}
