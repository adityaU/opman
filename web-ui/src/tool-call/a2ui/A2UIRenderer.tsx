import React, { useEffect, useRef, useMemo } from "react";
import mermaid from "mermaid";
import { extractBlocks, esc } from "./types";
import type { A2UIBlock } from "./types";
import { blocksToHtml } from "./render";

let mermaidInited = false;

interface A2UIRendererProps {
  input: unknown;
}

/** Fire a custom event on window for the app layer to handle callbacks. */
function fireCallback(callbackId: string, payload: unknown) {
  const detail = JSON.stringify({ callback_id: callbackId, payload: payload ?? null });
  window.dispatchEvent(new CustomEvent("opman:a2ui-callback", { detail }));
}

/** Wire event delegation on the container for buttons, forms, toggles, and mermaid zoom. */
function wireEvents(el: HTMLElement) {
  // Button clicks
  el.addEventListener("click", (ev) => {
    const target = ev.target as HTMLElement;
    const btn = target.closest<HTMLElement>("[data-a2ui-callback]");
    if (!btn || btn.closest("form")) return;
    const cbId = btn.getAttribute("data-a2ui-callback");
    if (!cbId) return;
    fireCallback(cbId, null);
    btn.setAttribute("disabled", "true");
    btn.innerHTML = `<span class="a2ui-btn-done">✓ Sent</span>`;
  });

  // Form submissions
  el.addEventListener("submit", (ev) => {
    ev.preventDefault();
    const form = (ev.target as HTMLElement).closest<HTMLFormElement>("[data-a2ui-form-callback]");
    if (!form) return;
    const cbId = form.getAttribute("data-a2ui-form-callback");
    if (!cbId) return;
    const values: Record<string, string> = {};
    const inputs = form.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>("input, textarea");
    for (const inp of inputs) {
      if (inp.name) values[inp.name] = inp.value;
      inp.disabled = true;
    }
    fireCallback(cbId, values);
    const submitBtn = form.querySelector<HTMLButtonElement>("button[type=submit]");
    if (submitBtn) { submitBtn.disabled = true; submitBtn.textContent = "Submitted"; }
  });

  // Mermaid zoom
  el.addEventListener("click", (ev) => {
    const btn = (ev.target as HTMLElement).closest<HTMLElement>("[data-a2ui-mermaid-zoom]");
    if (!btn) return;
    const action = btn.getAttribute("data-a2ui-mermaid-zoom");
    const viewport = btn.closest(".a2ui-mermaid")?.querySelector<HTMLElement>(".a2ui-mermaid-viewport");
    if (!viewport) return;
    const cur = parseFloat(viewport.style.transform?.match(/scale\(([^)]+)\)/)?.[1] ?? "1");
    let next = cur;
    if (action === "in") next = Math.min(3, cur + 0.25);
    else if (action === "out") next = Math.max(0.25, cur - 0.25);
    else if (action === "reset") next = 1;
    viewport.style.transform = `scale(${next})`;
  });

  // Render mermaid diagrams
  runMermaid(el);
}

/** Initialize mermaid once, then render any <pre class="mermaid"> nodes. */
function runMermaid(el: HTMLElement) {
  const nodes = el.querySelectorAll<HTMLElement>("pre.mermaid");
  if (!nodes.length) return;
  if (!mermaidInited) {
    mermaid.initialize({ startOnLoad: false, theme: "dark", securityLevel: "strict" });
    mermaidInited = true;
  }
  mermaid.run({ nodes: Array.from(nodes) }).catch(() => { /* diagram syntax errors are non-fatal */ });
}

export function A2UIRenderer({ input }: A2UIRendererProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const { title, blocks } = useMemo(() => extractBlocks(input), [input]);

  const html = useMemo(() => {
    if (!blocks.length) return "";
    return blocksToHtml(blocks);
  }, [blocks]);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    wireEvents(el);
  }, [html]);

  if (!html) return <div className="a2ui-empty">No UI blocks</div>;

  return (
    <div className="a2ui-container" ref={containerRef}>
      {title && <div className="a2ui-title">{title}</div>}
      <div
        className="a2ui-blocks-inner"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  );
}
