import React, { useMemo } from "react";

/**
 * The `[ref=eN]` outline, rendered as the structure it is.
 *
 * The generic tool card shows this as a wall of preformatted text, which throws
 * away everything the format encodes: indentation is the page's hierarchy, the
 * first word is the element's role, the quoted part is what a person would read,
 * and the ref is the handle the agent clicked by. Splitting those four apart is
 * the whole reason this card exists — a reader scanning a transcript wants to
 * see *what the agent was looking at*, not parse it.
 */

export interface OutlineRow {
  readonly depth: number;
  readonly role: string;
  readonly name: string;
  readonly ref: string | null;
  readonly note: string;
}

/** Roles worth tinting. Everything else stays in the muted default. */
const ACTIONABLE = new Set([
  "link",
  "button",
  "textbox",
  "searchbox",
  "checkbox",
  "radio",
  "combobox",
  "switch",
  "menuitem",
  "tab",
  "option",
]);

const HEADINGS = new Set(["h1", "h2", "h3", "h4", "h5", "h6"]);

/**
 * Parse one outline line. Tolerant by design: an unparseable line still renders,
 * as its own text, rather than taking the card down with it.
 */
export function parseOutline(outline: string): readonly OutlineRow[] {
  return outline
    .split("\n")
    .filter((line) => line.trim().length > 0)
    .map((line) => {
      const depth = line.length - line.trimStart().length;
      const rest = line.trim();

      const refMatch = rest.match(/\[ref=(e\d+)\]/);
      const ref = refMatch ? refMatch[1] : null;

      const nameMatch = rest.match(/"([^"]*)"/);
      const name = nameMatch ? nameMatch[1] : "";

      const role = rest.split(/\s+/)[0] ?? "";

      // Whatever is left after role, name and ref — state like `checked`,
      // `value="…"`, or an href.
      const note = rest
        .replace(/\[ref=e\d+\]/, "")
        .replace(/"[^"]*"/, "")
        .replace(new RegExp(`^${escapeRegExp(role)}\\s*`), "")
        .trim();

      return { depth, role, name, ref, note };
    });
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** How many of these the agent could actually act on. */
export function actionableCount(rows: readonly OutlineRow[]): number {
  return rows.filter((row) => row.ref !== null).length;
}

export const OutlineView: React.FC<{ readonly outline: string }> = React.memo(
  function OutlineView({ outline }) {
    const rows = useMemo(() => parseOutline(outline), [outline]);
    if (rows.length === 0) return null;

    return (
      <div className="bwc-outline" role="list">
        {rows.map((row, index) => (
          <div
            key={index}
            role="listitem"
            className="bwc-row"
            // Indentation is the page's own hierarchy; capped so a deeply
            // nested node does not push its text off a narrow transcript.
            style={{ paddingLeft: `${Math.min(row.depth, 8) * 8}px` }}
          >
            <span className={roleClass(row.role)}>{row.role}</span>
            {row.name && <span className="bwc-name">{row.name}</span>}
            {row.note && <span className="bwc-note">{row.note}</span>}
            {row.ref && <span className="bwc-ref">{row.ref}</span>}
          </div>
        ))}
      </div>
    );
  },
);

function roleClass(role: string): string {
  if (ACTIONABLE.has(role)) return "bwc-role is-actionable";
  if (HEADINGS.has(role)) return "bwc-role is-heading";
  if (role === "text") return "bwc-role is-text";
  return "bwc-role";
}
