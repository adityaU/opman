import React, { useMemo } from "react";

/**
 * A project's colour, and the mark that shows it.
 *
 * The colour answers "which project is this" across a six-pane grid without
 * the eye having to read anything, and it tints the pane card's own border so
 * it survives zen mode. It is derived from the path rather than assigned, so
 * it is stable across reloads and machines with no registry to keep in sync.
 *
 * Hue only. Lightness and chroma come from tokens so the same hue reads
 * correctly on a dark canvas and a light one, which an rgb hash could not do.
 *
 * The mark is a dot, not a lozenge of initials. `[op] opman` printed the same
 * fact twice, in two weights, in a 30px header — and the initials were the
 * weaker of the two, since they are a lossy rendering of the word sitting
 * right beside them. Initials survive only for the case that actually needs
 * them: no room for the name at all.
 */

function hashPath(path: string): number {
  // FNV-1a: cheap, no dependency, and spreads adjacent paths like /a and /b
  // into distant hues, which a sum-of-chars hash notoriously fails to do.
  let hash = 0x811c9dc5;
  for (let i = 0; i < path.length; i += 1) {
    hash ^= path.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

/** Yellow-green, where perceived lightness runs away from every other hue. */
const EXCLUDED_START = 95;
const EXCLUDED_WIDTH = 40;
const USABLE = 360 - EXCLUDED_WIDTH;

/**
 * A stable hue per project, skipping the yellow-green band: at a fixed OKLCH
 * lightness those hues read markedly brighter than the rest, so a project that
 * landed there would look like it was highlighted.
 */
export function projectHue(projectPath: string): number {
  const raw = hashPath(projectPath) % USABLE;
  return raw < EXCLUDED_START ? raw : raw + EXCLUDED_WIDTH;
}

export function projectColorVars(projectPath: string): React.CSSProperties {
  return { "--pane-hue": projectHue(projectPath) } as React.CSSProperties;
}

/** Two letters, from the last path segment — "opman" reads as "op". */
export function projectInitials(name: string): string {
  const cleaned = name.replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  const words = cleaned.split(/\s+/).filter(Boolean);
  if (words.length === 0) return "··";
  if (words.length === 1) return words[0].slice(0, 2).toLowerCase();
  return (words[0][0] + words[1][0]).toLowerCase();
}

interface ProjectBadgeProps {
  readonly projectPath: string;
  readonly name: string;
  /** An agent is working in this pane — the badge is where that reads. */
  readonly busy?: boolean;
  readonly showName?: boolean;
}

export const ProjectBadge: React.FC<ProjectBadgeProps> = React.memo(function ProjectBadge({
  projectPath,
  name,
  busy = false,
  showName = true,
}) {
  const initials = useMemo(() => projectInitials(name), [name]);
  return (
    <span
      className={`wsp-badge${busy ? " is-busy" : ""}`}
      style={projectColorVars(projectPath)}
      title={projectPath}
    >
      {showName ? (
        <span className="wsp-badge-dot" aria-hidden="true" />
      ) : (
        <span className="wsp-badge-chip" aria-hidden="true">
          {initials}
        </span>
      )}
      {showName && <span className="wsp-badge-name">{name}</span>}
      {busy && <span className="wsp-sr-only">busy</span>}
    </span>
  );
});
