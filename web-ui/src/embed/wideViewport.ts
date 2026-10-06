/**
 * Make an embed lay out as desktop at any width.
 *
 * An embed's viewport is its pane, and panes are routinely narrower than the
 * 768px phone breakpoint. Left alone, every `@media (max-width: …)` rule would
 * switch the widget to its phone layout — mobile composer, mobile dock — inside
 * a desktop pane that the same widget, shown locally, would never get. A pane
 * on a wide screen is a desktop surface, so the embed answers width queries as
 * a wide screen would: `max-width` never matches, `min-width` always does.
 *
 * Done by rewriting the media lists of the loaded stylesheets in place — and of
 * any added later, since lazy chunks bring their own CSS. Only width conditions
 * are touched; `prefers-reduced-motion`, `hover` and the rest still mean what
 * they say.
 */

const MAX_WIDTH = /\(\s*max-width\s*:[^)]*\)/i;
const MIN_WIDTH = /\(\s*min-width\s*:[^)]*\)/i;

/** The media text a wide viewport would make of `text`, or null to leave it. */
export function wideMediaText(text: string): string | null {
  if (MAX_WIDTH.test(text)) return "not all";
  if (!MIN_WIDTH.test(text)) return null;
  const rest = text.replace(new RegExp(MIN_WIDTH, "gi"), "").replace(/^\s*and\s+|\s+and\s*$/gi, "").trim();
  return rest || "all";
}

function rewriteRules(rules: CSSRuleList): void {
  for (const rule of Array.from(rules)) {
    if (!(rule instanceof CSSMediaRule)) continue;
    const next = wideMediaText(rule.media.mediaText);
    if (next !== null) rule.media.mediaText = next;
    rewriteRules(rule.cssRules);
  }
}

function rewriteSheet(sheet: CSSStyleSheet): void {
  try {
    rewriteRules(sheet.cssRules);
  } catch {
    // A cross-origin sheet (a font provider) has no rules we may read, and no
    // layout rules of ours either.
  }
}

function rewriteAll(): void {
  for (const sheet of Array.from(document.styleSheets)) rewriteSheet(sheet);
}

export function emulateWideViewport(): void {
  rewriteAll();
  // A <link> is parsed after it is inserted, so listen for its load as well as
  // for the insertion; a <style> is ready at once.
  const observer = new MutationObserver((records) => {
    for (const record of records) {
      for (const node of Array.from(record.addedNodes)) {
        if (node instanceof HTMLLinkElement) node.addEventListener("load", rewriteAll, { once: true });
      }
    }
    rewriteAll();
  });
  observer.observe(document.head, { childList: true, subtree: true, characterData: true });
}
