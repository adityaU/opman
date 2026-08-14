// Main-content text extraction, for when the LLM wants to *read* rather than act.
//
// A Readability-shaped heuristic without the dependency: score block containers by the
// text they hold minus the text that is inside links, which is what separates an article
// body from a nav column. Returning one container instead of document.body is the
// difference between a few hundred tokens and the whole chrome of the site on every read.
(() => {
  const maxChars = MAX_CHARS;
  const NOISE = /(^|[\s-_])(nav|menu|sidebar|footer|header|banner|advert|promo|cookie|comment|related|share|social)([\s-_]|$)/i;

  const linkTextLength = (el) => {
    let total = 0;
    for (const a of el.querySelectorAll('a')) total += (a.innerText || '').length;
    return total;
  };

  const score = (el) => {
    const text = el.innerText || '';
    if (text.length < 140) return -1;
    const id = `${el.id} ${el.className}`;
    if (typeof id === 'string' && NOISE.test(id)) return -1;
    // Link-dense blocks are navigation; paragraph-dense blocks are prose.
    const density = linkTextLength(el) / text.length;
    if (density > 0.5) return -1;
    return text.length * (1 - density) + el.querySelectorAll('p').length * 120;
  };

  let best = document.body;
  let bestScore = best ? score(best) : -1;
  const candidates = document.querySelectorAll('article, main, [role=main], section, div');
  for (const el of candidates) {
    const value = score(el);
    if (value > bestScore) {
      best = el;
      bestScore = value;
    }
  }

  const text = ((best && best.innerText) || '')
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
