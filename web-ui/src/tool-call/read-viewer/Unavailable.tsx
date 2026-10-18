import React from "react";
import { FileX } from "lucide-react";

/** Shown when a file cannot be loaded, e.g. it lives on another machine. */
export function Unavailable({ message, href }: { message?: string; href?: string | null }) {
  return (
    <div className="rv-state rv-state-error" role="status">
      <FileX size={14} aria-hidden="true" />
      <span>{message ?? "File not available on this server"}</span>
      {href && <a href={href} download>Try download</a>}
    </div>
  );
}

export function Loading({ label = "Loading…" }: { label?: string }) {
  return <div className="rv-state" role="status"><span className="rv-spinner" aria-hidden="true" />{label}</div>;
}
