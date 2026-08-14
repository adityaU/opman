// Page → compact semantic outline. This is the whole token-minimisation story: the LLM
// never sees HTML, only the interactive and structural nodes that a person could act on,
// each interactive one tagged with a [ref=eN] handle that stays valid until the next
// snapshot. A page whose HTML is 400 KB typically lands here under 2 KB.
//
// Evaluated by Page.evaluate with OPTIONS substituted. Returns a JSON string so the CDP
// layer never has to walk a remote object graph.
(() => {
  const opts = OPTIONS;
  const maxNodes = opts.maxNodes;
  const maxChars = opts.maxChars;
  const maxTextLen = opts.maxTextLen;
  const viewportOnly = opts.viewportOnly;

  // Refs live on the window so a later click/type can resolve eN without re-walking.
  // A fresh array per snapshot is what makes stale refs fail loudly instead of hitting
  // whatever element inherited the index.
  const refs = [];
  window.__opmanRefs = refs;

  const INTERACTIVE_TAGS = new Set([
    'A', 'BUTTON', 'INPUT', 'SELECT', 'TEXTAREA', 'SUMMARY', 'OPTION', 'LABEL',
  ]);
  const INTERACTIVE_ROLES = new Set([
    'button', 'link', 'checkbox', 'radio', 'textbox', 'searchbox', 'combobox',
    'listbox', 'option', 'menuitem', 'menuitemcheckbox', 'menuitemradio', 'tab',
    'switch', 'slider', 'spinbutton', 'treeitem',
  ]);
  const LANDMARK_TAGS = new Set(['MAIN', 'NAV', 'HEADER', 'FOOTER', 'ASIDE', 'FORM', 'DIALOG']);
  const HEADING_TAGS = new Set(['H1', 'H2', 'H3', 'H4', 'H5', 'H6']);
  const SKIP_TAGS = new Set([
    'SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE', 'HEAD', 'META', 'LINK', 'SVG', 'PATH', 'BR',
  ]);
  const TEXT_TAGS = new Set(['P', 'LI', 'TD', 'TH', 'DT', 'DD', 'BLOCKQUOTE', 'FIGCAPTION', 'PRE']);

  const clip = (s, n) => {
    const t = (s || '').replace(/\s+/g, ' ').trim();
    return t.length > n ? t.slice(0, n) + '…' : t;
  };

  const visible = (el) => {
    const style = getComputedStyle(el);
    if (style.visibility === 'hidden' || style.display === 'none' || style.opacity === '0') return false;
    if (el.getAttribute('aria-hidden') === 'true') return false;
    const rect = el.getBoundingClientRect();
    if (rect.width < 1 || rect.height < 1) return false;
    if (viewportOnly && (rect.bottom < 0 || rect.top > innerHeight)) return false;
    return true;
  };

  // Accessible name, in the order a screen reader would resolve it. innerText is the
  // last resort and is capped hard — an unlabelled <div role=button> wrapping a whole
  // card would otherwise drag the card's entire text into one line.
  const nameOf = (el) => {
    const labelledBy = el.getAttribute('aria-labelledby');
    if (labelledBy) {
      const parts = labelledBy.split(/\s+/)
        .map((id) => document.getElementById(id))
        .filter(Boolean)
        .map((n) => n.innerText);
      if (parts.length) return clip(parts.join(' '), maxTextLen);
    }
    const direct = el.getAttribute('aria-label')
      || el.getAttribute('alt')
      || el.getAttribute('placeholder')
      || el.getAttribute('title');
    if (direct) return clip(direct, maxTextLen);
    if (el.tagName === 'INPUT' && el.labels && el.labels.length) {
      return clip(el.labels[0].innerText, maxTextLen);
    }
    if (el.tagName === 'INPUT' && (el.type === 'submit' || el.type === 'button')) {
      return clip(el.value, maxTextLen);
    }
    return clip(el.innerText || el.textContent, maxTextLen);
  };

  const roleOf = (el) => {
    const explicit = el.getAttribute('role');
    if (explicit) return explicit;
    switch (el.tagName) {
      case 'A': return el.hasAttribute('href') ? 'link' : 'generic';
      case 'BUTTON': case 'SUMMARY': return 'button';
      case 'SELECT': return 'combobox';
      case 'TEXTAREA': return 'textbox';
      case 'INPUT': {
        const type = (el.type || 'text').toLowerCase();
        if (type === 'checkbox' || type === 'radio') return type;
        if (type === 'submit' || type === 'button' || type === 'reset') return 'button';
        if (type === 'search') return 'searchbox';
        return 'textbox';
      }
      default: return el.tagName.toLowerCase();
    }
  };

  const isInteractive = (el) => {
    if (el.disabled) return false;
    if (INTERACTIVE_TAGS.has(el.tagName)) return el.tagName !== 'A' || el.hasAttribute('href');
    if (INTERACTIVE_ROLES.has(el.getAttribute('role'))) return true;
    if (el.isContentEditable) return true;
    if (el.hasAttribute('onclick')) return true;
    const tabindex = el.getAttribute('tabindex');
    return tabindex !== null && tabindex !== '-1';
  };

  // Element state worth a token: anything that changes what the next action should be.
  const stateOf = (el, role) => {
    const bits = [];
    if (role === 'textbox' || role === 'searchbox') {
      const value = el.type === 'password' ? (el.value ? '••••' : '') : clip(el.value, 40);
      if (value) bits.push(`value="${value}"`);
      if (el.required) bits.push('required');
    }
    if (role === 'checkbox' || role === 'radio' || role === 'switch') {
      bits.push(el.checked ? 'checked' : 'unchecked');
    }
    if (role === 'combobox' && el.selectedOptions && el.selectedOptions.length) {
      bits.push(`selected="${clip(el.selectedOptions[0].text, 40)}"`);
    }
    const expanded = el.getAttribute('aria-expanded');
    if (expanded) bits.push(`expanded=${expanded}`);
    if (el.getAttribute('aria-current')) bits.push('current');
    if (el === document.activeElement) bits.push('focused');
    return bits;
  };

  const lines = [];
  let chars = 0;
  let truncated = false;
  const seenText = new Set();

  const emit = (depth, text) => {
    if (lines.length >= maxNodes || chars >= maxChars) {
      truncated = true;
      return false;
    }
    const line = ' '.repeat(Math.min(depth, 8)) + text;
    lines.push(line);
    chars += line.length + 1;
    return true;
  };

  const walk = (el, depth) => {
    if (truncated) return;
    if (SKIP_TAGS.has(el.tagName)) return;
    if (!visible(el)) return;

    let nextDepth = depth;
    const role = roleOf(el);

    if (isInteractive(el)) {
      const ref = `e${refs.length}`;
      refs.push(el);
      const name = nameOf(el);
      const state = stateOf(el, role);
      const href = el.tagName === 'A' ? clip(el.getAttribute('href'), 60) : '';
      const parts = [role];
      if (name) parts.push(`"${name}"`);
      parts.push(`[ref=${ref}]`);
      if (href && !href.startsWith('javascript:')) parts.push(`→${href}`);
      if (state.length) parts.push(state.join(' '));
      if (!emit(depth, parts.join(' '))) return;
      nextDepth = depth + 1;
    } else if (HEADING_TAGS.has(el.tagName)) {
      const name = nameOf(el);
      if (name) emit(depth, `${el.tagName.toLowerCase()} "${name}"`);
      nextDepth = depth + 1;
    } else if (LANDMARK_TAGS.has(el.tagName)) {
      const label = el.getAttribute('aria-label');
      emit(depth, label ? `${role} "${clip(label, maxTextLen)}"` : role);
      nextDepth = depth + 1;
    } else if (el.tagName === 'IMG') {
      const alt = nameOf(el);
      if (alt) emit(depth, `image "${alt}"`);
      return;
    } else if (TEXT_TAGS.has(el.tagName) && !el.querySelector('a,button,input,select,textarea')) {
      // Leaf prose only; a <li> that contains a link is left to its children so the
      // link keeps its ref instead of being flattened into an untargetable string.
      const text = clip(el.innerText, maxTextLen);
      if (text.length > 1 && !seenText.has(text)) {
        seenText.add(text);
        emit(depth, `text "${text}"`);
      }
      return;
    }

    for (const child of el.children) walk(child, nextDepth);
  };

  if (document.body) walk(document.body, 0);

  return JSON.stringify({
    url: location.href,
    title: document.title,
    scrollY: Math.round(scrollY),
    scrollHeight: Math.round(document.documentElement.scrollHeight),
    viewportHeight: Math.round(innerHeight),
    refCount: refs.length,
    truncated,
    outline: lines.join('\n'),
  });
})()
