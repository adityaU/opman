/**
 * CodeMirror 6 theme and syntax highlighting configuration.
 * Uses the app's CSS variables so editor colors stay in sync with the active theme.
 */
import { HighlightStyle, syntaxHighlighting, LanguageDescription } from "@codemirror/language";
import { languages as languageData } from "@codemirror/language-data";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

// ── Language extension loading ──────────────────────────

export async function loadLanguageExtension(path: string, lang: string) {
  const byPath = LanguageDescription.matchFilename(languageData, path);
  if (byPath) return byPath.load();
  const normalized = lang.toLowerCase();
  const byName = languageData.find(
    (entry) => entry.name.toLowerCase() === normalized || entry.alias.includes(normalized),
  );
  return byName ? byName.load() : null;
}

// ── Editor chrome ───────────────────────────────────────

const editorTheme = EditorView.theme(
  {
    "&": {
      height: "100%",
      fontSize: "13px",
      backgroundColor: "var(--color-bg)",
      color: "var(--color-text)",
    },
    ".cm-scroller": { overflow: "auto", fontFamily: "var(--font-mono, monospace)" },
    ".cm-content": { caretColor: "var(--color-primary)" },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--color-primary)" },
    ".cm-gutters": {
      backgroundColor: "var(--color-bg)",
      color: "var(--color-text-muted)",
      borderRight: "1px solid var(--color-border-subtle)",
    },
    ".cm-activeLineGutter": {
      backgroundColor: "var(--theme-surface-hover, var(--color-bg-hover))",
      color: "var(--color-text)",
    },
    ".cm-activeLine": {
      backgroundColor: "var(--theme-surface-3, var(--color-bg-element))",
    },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
      backgroundColor: "var(--color-bg-element, #1a1a1a)",
    },
    ".cm-selectionMatch": {
      backgroundColor: "var(--theme-surface-hover, var(--color-bg-hover))",
    },
    ".cm-matchingBracket": {
      backgroundColor: "var(--theme-surface-hover, var(--color-bg-hover))",
      outline: "1px solid var(--color-border)",
    },
    ".cm-foldGutter .cm-gutterElement": { color: "var(--color-text-muted)" },
    ".cm-foldPlaceholder": {
      backgroundColor: "var(--color-bg-element)",
      border: "1px solid var(--color-border-subtle)",
      color: "var(--color-text-muted)",
    },
    ".cm-tooltip": {
      backgroundColor: "var(--color-bg-panel)",
      border: "1px solid var(--color-border)",
      color: "var(--color-text)",
    },
    ".cm-tooltip-autocomplete": {
      "& > ul > li[aria-selected]": { backgroundColor: "var(--color-bg-element)" },
    },
    ".cm-searchMatch": {
      backgroundColor: "var(--theme-primary-soft)",
      outline: "1px solid var(--theme-primary-border)",
    },
    ".cm-searchMatch.cm-searchMatch-selected": {
      backgroundColor: "color-mix(in srgb, var(--color-primary) 18%, var(--color-bg-element))",
    },
  },
  { dark: true },
);

// ── Syntax highlighting ─────────────────────────────────

const editorHighlightStyle = HighlightStyle.define([
  { tag: [tags.keyword, tags.modifier, tags.operatorKeyword],
    color: "var(--color-syntax-keyword, var(--color-primary))" },
  { tag: [tags.function(tags.variableName), tags.function(tags.definition(tags.variableName))],
    color: "var(--color-syntax-function, var(--color-accent))" },
  { tag: [tags.definition(tags.typeName), tags.typeName, tags.className, tags.namespace],
    color: "var(--color-syntax-tag, var(--color-secondary))" },
  { tag: [tags.string, tags.special(tags.string), tags.character],
    color: "var(--color-syntax-string, var(--color-success))" },
  { tag: [tags.number, tags.integer, tags.float, tags.bool],
    color: "var(--color-syntax-number, var(--color-warning))" },
  { tag: [tags.comment, tags.lineComment, tags.blockComment],
    color: "var(--color-syntax-comment, var(--color-text-muted))", fontStyle: "italic" },
  { tag: [tags.operator, tags.compareOperator, tags.arithmeticOperator, tags.logicOperator, tags.updateOperator],
    color: "var(--color-syntax-operator, var(--color-text))" },
  { tag: [tags.punctuation, tags.separator, tags.bracket, tags.angleBracket, tags.squareBracket, tags.paren, tags.brace],
    color: "var(--color-syntax-punctuation, var(--color-text-muted))" },
  { tag: [tags.tagName, tags.standard(tags.tagName)],
    color: "var(--color-syntax-tag, var(--color-secondary))" },
  { tag: [tags.attributeName],
    color: "var(--color-syntax-attribute, var(--color-info))" },
  { tag: [tags.attributeValue],
    color: "var(--color-syntax-string, var(--color-success))" },
  { tag: [tags.regexp],
    color: "var(--color-syntax-regex, var(--color-error))" },
  { tag: [tags.variableName], color: "var(--color-text)" },
  { tag: [tags.propertyName, tags.definition(tags.propertyName)],
    color: "var(--color-syntax-attribute, var(--color-info))" },
  { tag: [tags.self, tags.null],
    color: "var(--color-syntax-keyword, var(--color-primary))" },
  { tag: [tags.escape],
    color: "var(--color-syntax-regex, var(--color-error))" },
  { tag: [tags.heading], color: "var(--color-primary)", fontWeight: "bold" },
  { tag: [tags.link, tags.url], color: "var(--color-secondary)", textDecoration: "underline" },
  { tag: [tags.emphasis], fontStyle: "italic" },
  { tag: [tags.strong], fontWeight: "bold" },
  { tag: [tags.meta, tags.annotation, tags.processingInstruction],
    color: "var(--color-syntax-comment, var(--color-text-muted))" },
  { tag: [tags.invalid], color: "var(--color-error)", textDecoration: "underline wavy" },
]);

/** Combined theme extension: chrome + syntax highlighting. */
export const editorThemeExtension = [editorTheme, syntaxHighlighting(editorHighlightStyle)];
