// Resolve a [ref=eN] handle from the last snapshot into viewport coordinates.
//
// Actions are then dispatched as real CDP input events at those coordinates rather than
// as `el.click()`, so pages that listen for pointer events, hover state, or focus order
// behave the same as they would for a person. Scrolling into view first is what makes a
// ref taken from a long outline actually clickable.
(() => {
  const ref = REF;
  const refs = window.__opmanRefs;
  if (!refs) {
    return JSON.stringify({ error: 'no snapshot on this page yet — take a snapshot first' });
  }
  const index = Number.parseInt(String(ref).replace(/^e/, ''), 10);
  const el = Number.isNaN(index) ? null : refs[index];
  if (!el) {
    return JSON.stringify({ error: `ref ${ref} is not on this page — the page changed, re-snapshot` });
  }
  if (!el.isConnected) {
    return JSON.stringify({ error: `ref ${ref} was removed from the page — re-snapshot` });
  }

  el.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
  const rect = el.getBoundingClientRect();
  if (rect.width < 1 || rect.height < 1) {
    return JSON.stringify({ error: `ref ${ref} is not visible` });
  }

  // Focus here rather than relying on the synthetic click: text inputs need focus before
  // Input.insertText, and a click alone can be swallowed by an overlay.
  if (typeof el.focus === 'function') el.focus({ preventScroll: true });

  return JSON.stringify({
    x: Math.round(rect.left + rect.width / 2),
    y: Math.round(rect.top + rect.height / 2),
    tag: el.tagName.toLowerCase(),
    editable: el.isContentEditable
      || el.tagName === 'TEXTAREA'
      || (el.tagName === 'INPUT' && !['checkbox', 'radio', 'submit', 'button'].includes((el.type || '').toLowerCase())),
    select: el.tagName === 'SELECT',
  });
})()
