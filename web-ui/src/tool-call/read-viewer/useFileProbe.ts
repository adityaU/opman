import { useEffect, useState } from "react";

export type Probe =
  | { readonly state: "checking" }
  | { readonly state: "ready"; readonly size: number | null }
  | { readonly state: "missing" };

/**
 * Ask the server for one byte of the file, to learn whether it exists and how big it is.
 * Frames and sandboxes cannot report a failed load, so this is how they get an error state.
 */
export function useFileProbe(url: string): Probe {
  const [probe, setProbe] = useState<Probe>({ state: "checking" });
  useEffect(() => {
    const abort = new AbortController();
    setProbe({ state: "checking" });
    fetch(url, { headers: { Range: "bytes=0-0" }, signal: abort.signal })
      .then((res) => {
        if (!res.ok) return setProbe({ state: "missing" });
        const range = res.headers.get("content-range");
        const total = range ? Number(range.split("/")[1]) : Number(res.headers.get("content-length"));
        setProbe({ state: "ready", size: Number.isFinite(total) && total > 0 ? total : null });
        abort.abort();
      })
      .catch(() => { if (!abort.signal.aborted) setProbe({ state: "missing" }); });
    return () => abort.abort();
  }, [url]);
  return probe;
}
