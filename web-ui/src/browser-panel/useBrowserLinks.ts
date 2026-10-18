import { useEffect } from "react";

/** A single listener covers markdown, tool results, settings, and streamed UI. */
export function useBrowserLinks(project: string, open: (project: string, url: string) => boolean) {
  useEffect(() => {
    const click = (event: MouseEvent) => {
      if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      const target = event.target;
      if (!(target instanceof Element)) return;
      const anchor = target.closest<HTMLAnchorElement>("a[href]");
      if (!anchor || anchor.hasAttribute("download") || anchor.closest(".bwp-urlbar") ||
        anchor.dataset.openExternal === "true") return;
      const raw = anchor.getAttribute("href");
      if (!raw || !/^(https?:)?\/\//i.test(raw)) return;
      if (!open(project, anchor.href)) return;
      event.preventDefault();
      event.stopPropagation();
    };
    document.addEventListener("click", click, true);
    return () => document.removeEventListener("click", click, true);
  }, [project, open]);
}
