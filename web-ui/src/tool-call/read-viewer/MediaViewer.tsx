import React, { useEffect, useState } from "react";
import { Loading, Unavailable } from "./Unavailable";

interface MediaViewerProps {
  readonly url: string;
  readonly video: boolean;
  readonly onMeta: (text: string) => void;
}

function clock(seconds: number): string {
  if (!Number.isFinite(seconds)) return "";
  const total = Math.round(seconds);
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

/** Native video or audio controls. Only metadata is fetched until the user plays. */
export function MediaViewer({ url, video, onMeta }: MediaViewerProps) {
  const [phase, setPhase] = useState<"loading" | "ready" | "failed">("loading");
  useEffect(() => setPhase("loading"), [url]);

  const common = {
    controls: true,
    preload: "metadata" as const,
    src: url,
    onError: () => setPhase("failed"),
  };
  if (phase === "failed") return <Unavailable message="Cannot play this file here" />;

  const loaded = (e: React.SyntheticEvent<HTMLMediaElement>) => {
    setPhase("ready");
    const el = e.currentTarget;
    const size = el instanceof HTMLVideoElement && el.videoWidth ? `${el.videoWidth} × ${el.videoHeight} · ` : "";
    onMeta(`${size}${clock(el.duration)}`);
  };

  return (
    <div className={video ? "rv-video-wrap" : "rv-audio-wrap"}>
      {phase === "loading" && <Loading />}
      {video ? (
        <video className={`rv-video${phase === "ready" ? "" : " rv-hidden"}`} playsInline
          onLoadedMetadata={loaded} {...common} />
      ) : (
        <audio className={`rv-audio${phase === "ready" ? "" : " rv-hidden"}`}
          onLoadedMetadata={loaded} {...common} />
      )}
    </div>
  );
}
