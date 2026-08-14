import type { EditorGotoKind, EditorLspActions } from "../../api";
import { markdownElement } from "./lspMarkdown";

/**
 * The hover card.
 *
 * A hover used to be a paragraph you could only read. It is also the one moment
 * the editor knows exactly which symbol you mean, so it carries the things you
 * would otherwise go and find: where this is defined, what its type is, who
 * else uses it. Every one of those is two controls, not one — going there and
 * choosing where "there" is — because jumping to a definition is as often
 * "beside what I am reading" as "instead of it".
 *
 * Only actions the language server actually claims are drawn. An action that
 * resolves to an empty panel teaches the reader to stop trusting the row.
 */

export type HoverAction = EditorGotoKind | "references" | "rename";

export interface HoverActionRequest {
  readonly action: HoverAction;
  /** `here` replaces the current view; `choose` asks which pane through the overlay. */
  readonly where: "here" | "choose";
}

export interface HoverCardOptions {
  /** The server's markdown: a fenced signature, then prose. */
  readonly text: string;
  readonly actions: EditorLspActions;
  readonly onAction: (request: HoverActionRequest) => void;
}

const NAVIGATIONS: readonly { readonly action: HoverAction; readonly label: string }[] = [
  { action: "definition", label: "Definition" },
  { action: "type-definition", label: "Type" },
  { action: "implementation", label: "Implementation" },
  { action: "declaration", label: "Declaration" },
  { action: "references", label: "References" },
];

function supports(actions: EditorLspActions, action: HoverAction): boolean {
  switch (action) {
    case "definition": return actions.definition;
    case "type-definition": return actions.typeDefinition;
    case "implementation": return actions.implementation;
    case "declaration": return actions.declaration;
    case "references": return actions.references;
    case "rename": return actions.rename;
  }
}

function element(tag: string, className: string, text?: string): HTMLElement {
  const node = document.createElement(tag);
  node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

/**
 * The "choose a pane" glyph: a pane divided, with the far half filled. Drawn
 * rather than borrowed from a font so it carries the same 1.75px stroke as the
 * fold chevron and the explorer's own icons.
 */
function splitIcon(): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 16 16");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("focusable", "false");
  const frame = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  frame.setAttribute("x", "2");
  frame.setAttribute("y", "3");
  frame.setAttribute("width", "12");
  frame.setAttribute("height", "10");
  frame.setAttribute("rx", "2");
  frame.setAttribute("fill", "none");
  frame.setAttribute("stroke", "currentColor");
  frame.setAttribute("stroke-width", "1.25");
  // A divider, not a filled half: at twelve pixels a solid block loses its
  // rounded corners and reads as a smudge rather than as a pane.
  const divider = document.createElementNS("http://www.w3.org/2000/svg", "line");
  divider.setAttribute("x1", "9");
  divider.setAttribute("y1", "3");
  divider.setAttribute("x2", "9");
  divider.setAttribute("y2", "13");
  divider.setAttribute("stroke", "currentColor");
  divider.setAttribute("stroke-width", "1.25");
  svg.append(frame, divider);
  return svg;
}

/** The signature line the server fenced, lifted out of the prose. */
function splitSignature(text: string): { signature: string | null; body: string } {
  const fence = /^```[\w-]*\n([\s\S]*?)\n?```\s*/;
  const match = fence.exec(text.trim());
  if (!match) return { signature: null, body: text };
  const signature = match[1].trim();
  const body = text.trim().slice(match[0].length).trim();
  // A one-line signature is a heading. A fenced block of real code is content,
  // and hoisting it would strip the highlighting the markdown renderer gives it.
  if (signature.includes("\n")) return { signature: null, body: text };
  return { signature, body };
}

function actionGroup(
  action: HoverAction,
  label: string,
  onAction: (request: HoverActionRequest) => void,
): HTMLElement {
  const group = element("span", "lsph-action");

  const go = element("button", "lsph-action-go", label);
  go.setAttribute("type", "button");
  go.addEventListener("mousedown", (event) => event.preventDefault());
  go.addEventListener("click", () => onAction({ action, where: "here" }));

  // References open a list rather than a file, so there is no pane to choose
  // until a row in that list is clicked — which offers the same choice again.
  if (action === "references") {
    group.append(go);
    return group;
  }

  const where = element("button", "lsph-action-where");
  where.setAttribute("type", "button");
  where.setAttribute("aria-label", `${label} in another pane`);
  where.title = `${label} in another pane, split or window`;
  where.append(splitIcon());
  where.addEventListener("mousedown", (event) => event.preventDefault());
  where.addEventListener("click", () => onAction({ action, where: "choose" }));

  group.append(go, where);
  return group;
}

export function hoverCard(options: HoverCardOptions): HTMLElement {
  const { text, actions, onAction } = options;
  const card = element("div", "lsph-card modal-popover-surface");

  const { signature, body } = splitSignature(text);
  if (signature) card.append(element("code", "lsph-signature", signature));
  if (body) {
    const doc = markdownElement(body, "lsph-doc");
    card.append(doc);
  }

  const available = NAVIGATIONS.filter((entry) => supports(actions, entry.action));
  const canRename = supports(actions, "rename");
  if (available.length === 0 && !canRename) return card;

  const rail = element("div", "lsph-rail");
  rail.setAttribute("role", "group");
  rail.setAttribute("aria-label", "Symbol actions");
  for (const entry of available) rail.append(actionGroup(entry.action, entry.label, onAction));
  if (canRename) {
    const rename = element("button", "lsph-action-go is-rename", "Rename");
    rename.setAttribute("type", "button");
    rename.addEventListener("mousedown", (event) => event.preventDefault());
    rename.addEventListener("click", () => onAction({ action: "rename", where: "here" }));
    rail.append(rename);
  }
  card.append(rail);
  return card;
}

/** The label the pane chooser shows while it waits for an answer. */
export function actionLabel(action: HoverAction, symbol: string): string {
  const name = NAVIGATIONS.find((entry) => entry.action === action)?.label ?? "Definition";
  return symbol ? `${name} · ${symbol}` : name;
}
