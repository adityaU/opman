/**
 * fileGlyph — maps a filename to the mark and hue its row wears in the tree.
 *
 * A tree where every file carries the same outline icon is a tree you have to
 * read word by word. Giving each extension a stable colour and a two-or-three
 * letter mark lets the eye find "the TypeScript one" or "the config one"
 * before it has read a single name.
 *
 * Hues are fixed for the families people actually scan for, and derived from
 * the extension string otherwise, so an unknown type still gets a consistent
 * identity across sessions instead of falling back to grey.
 */

export interface FileGlyph {
  /** Short mark rendered inside the tile — 1-3 characters. */
  mark: string;
  /** Hue angle used for the tile's tint and text. */
  hue: number;
}

/** Extensions whose colour is worth pinning rather than hashing. */
const PINNED: Record<string, [string, number]> = {
  ts: ["TS", 212],
  tsx: ["TSX", 199],
  js: ["JS", 48],
  jsx: ["JSX", 40],
  mjs: ["JS", 48],
  cjs: ["JS", 48],
  json: ["{ }", 32],
  css: ["CSS", 265],
  scss: ["SCS", 330],
  html: ["<>", 18],
  md: ["MD", 205],
  mdx: ["MDX", 205],
  rs: ["RS", 20],
  py: ["PY", 218],
  go: ["GO", 190],
  rb: ["RB", 355],
  java: ["JV", 8],
  c: ["C", 205],
  h: ["H", 262],
  cpp: ["C++", 205],
  sh: ["SH", 142],
  bash: ["SH", 142],
  zsh: ["SH", 142],
  toml: ["TML", 28],
  yaml: ["YML", 285],
  yml: ["YML", 285],
  lock: ["LCK", 0],
  sql: ["SQL", 172],
  svg: ["SVG", 92],
  png: ["IMG", 92],
  jpg: ["IMG", 92],
  jpeg: ["IMG", 92],
  gif: ["IMG", 92],
  webp: ["IMG", 92],
  pdf: ["PDF", 2],
  csv: ["CSV", 130],
  xlsx: ["XLS", 130],
  zip: ["ZIP", 44],
  tar: ["TAR", 44],
  gz: ["GZ", 44],
  txt: ["TXT", 220],
  log: ["LOG", 220],
  env: ["ENV", 52],
};

/** Dotfiles that read as project furniture rather than as their extension. */
const DOTFILES: Record<string, [string, number]> = {
  ".gitignore": ["GIT", 14],
  ".gitattributes": ["GIT", 14],
  ".env": ["ENV", 52],
  ".editorconfig": ["CFG", 220],
  ".npmrc": ["CFG", 220],
  ".dockerignore": ["DKR", 205],
  dockerfile: ["DKR", 205],
  makefile: ["MK", 30],
  "cargo.toml": ["TML", 28],
  "package.json": ["{ }", 32],
  "readme.md": ["MD", 205],
  "license": ["LIC", 45],
};

/** Deterministic hue for extensions with no pinned identity. */
function hashHue(seed: string): number {
  let h = 0;
  for (let i = 0; i < seed.length; i += 1) h = (h * 31 + seed.charCodeAt(i)) % 360;
  return h;
}

export function fileGlyph(name: string): FileGlyph {
  const lower = name.toLowerCase();

  const named = DOTFILES[lower];
  if (named) return { mark: named[0], hue: named[1] };

  const dot = lower.lastIndexOf(".");
  if (dot <= 0 || dot === lower.length - 1) {
    return { mark: lower.slice(0, 2).toUpperCase() || "?", hue: hashHue(lower) };
  }

  const ext = lower.slice(dot + 1);
  const pinned = PINNED[ext];
  if (pinned) return { mark: pinned[0], hue: pinned[1] };

  return { mark: ext.slice(0, 3).toUpperCase(), hue: hashHue(ext) };
}
