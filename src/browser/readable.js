// Main-content text extraction, for when the LLM wants to *read* rather than act.
//
// A Readability-shaped heuristic without the dependency: score block containers by the
// text they hold minus the text that is inside links, which is what separates an article
// body from a nav column. Returning one container instead of the whole document is the
// difference between a few hundred tokens and the entire chrome of the site on every read.
//
// Deliberately does not use `innerText`. `innerText` is defined in terms of *rendered*
// text, and an element that is not being rendered — a tab whose window has not been mapped
// yet, which is the normal state for a beat after a pane opens — falls back to
// `textContent`, stylesheet bodies and all. That produced pages of CSS instead of prose.
// Walking the tree here costs one pass and cannot degrade that way.
(() => {
  const maxChars = MAX_CHARS;
  const NOISE = /(^|[\s-_])(nav|menu|sidebar|footer|header|banner|advert|promo|cookie|comment|related|share|social)([\s-_]|$)/i;

  // Never contribute text: either not prose (script, style) or not this document.
  const SKIP = new Set([
    'SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE', 'HEAD', 'TITLE', 'META', 'LINK',
    'SVG', 'CANVAS', 'IFRAME', 'OBJECT', 'AUDIO', 'VIDEO', 'MAP',
  ]);

  // Tags that start a new line in the output, so a paragraph list does not run together.
  const BLOCK = new Set([
    'ADDRESS', 'ARTICLE', 'ASIDE', 'BLOCKQUOTE', 'BR', 'DD', 'DIV', 'DL', 'DT',
    'FIELDSET', 'FIGCAPTION', 'FIGURE', 'FOOTER', 'FORM', 'H1', 'H2', 'H3', 'H4', 'H5',
    'H6', 'HEADER', 'HR', 'LI', 'MAIN', 'NAV', 'OL', 'P', 'PRE', 'SECTION', 'TABLE',
    'TD', 'TH', 'TR', 'UL',
  ]);

  // The element's computed `display`, or null when it contributes no text at all.
  // Computed style, not a bounding box: this must stay independent of layout.
  const displayOf = (el) => {
    if (SKIP.has(el.tagName)) return null;
    if (el.hidden || el.getAttribute('aria-hidden') === 'true') return null;
    const style = window.getComputedStyle(el);
    if (!style) return null;
    const display = style.display;
    if (display === 'none' || style.visibility === 'hidden') return null;
    return display;
  };

  // Whether this element ends the current line. Tag names alone are not enough — sites
  // lay anchors out as blocks with no whitespace between them, and joining those gives
  // "Sign inImagesVideos".
  const breaks = (el, display) =>
    BLOCK.has(el.tagName) || (display !== 'contents' && !display.startsWith('inline'));

  // One bottom-up pass. Every element learns how much text it holds and how much of that
  // sits inside links, which is all the scoring needs — no per-candidate re-walking.
  const measure = (el, out) => {
    if (displayOf(el) === null) return { text: 0, link: 0 };
    let text = 0;
    let link = 0;
    for (const node of el.childNodes) {
      if (node.nodeType === Node.TEXT_NODE) {
        text += node.data.trim().length + 1;
        continue;
      }
      if (node.nodeType !== Node.ELEMENT_NODE) continue;
      const child = measure(node, out);
      text += child.text;
      link += child.link;
    }
    if (el.tagName === 'A') link = text;
    out.set(el, { text, link });
    return { text, link };
  };

  const measured = new Map();
  if (document.body) measure(document.body, measured);

  const score = (el) => {
    const size = measured.get(el);
    if (!size || size.text < 140) return -1;
    const name = `${el.id} ${el.className}`;
    if (typeof name === 'string' && NOISE.test(name)) return -1;
    // Link-dense blocks are navigation; paragraph-dense blocks are prose.
    const density = size.link / size.text;
    if (density > 0.5) return -1;
    return size.text * (1 - density) + el.querySelectorAll('p').length * 120;
  };

  let best = document.body;
  let bestScore = best ? score(best) : -1;
  for (const el of document.querySelectorAll('article, main, [role=main], section, div')) {
    const value = score(el);
    if (value > bestScore) {
      best = el;
      bestScore = value;
    }
  }

  // Second pass, once, over the winner only.
  const collect = (el, parts) => {
    const display = displayOf(el);
    if (display === null) return;
    const block = breaks(el, display);
    if (block) parts.push('\n');
    for (const node of el.childNodes) {
      if (node.nodeType === Node.TEXT_NODE) {
        const chunk = node.data.replace(/\s+/g, ' ');
        if (chunk.trim()) parts.push(chunk);
        continue;
      }
      if (node.nodeType === Node.ELEMENT_NODE) collect(node, parts);
    }
    if (block) parts.push('\n');
  };

  const parts = [];
  if (best) collect(best, parts);
  const text = parts
    .join('')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .join('\n');

  return JSON.stringify({
    url: location.href,
    title: document.title,
    truncated: text.length > maxChars,
    text: text.slice(0, maxChars),
  });
})()
