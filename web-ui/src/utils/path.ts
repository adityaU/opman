/**
 * Path fragments, for labelling.
 *
 * Written five times over across the app before this file existed — once per
 * panel that needed to show a file's name instead of its path — with two
 * different answers for a trailing slash. One copy, so "what do we call this
 * path" has one answer.
 */

/**
 * The last segment of a path.
 *
 * Trailing slashes are dropped rather than yielding an empty label: a project
 * root arrives as `/home/me/proj` from some callers and `/home/me/proj/` from
 * others, and both are that project. A path with no segments at all — `/`, or
 * the empty string — has nothing better to offer than itself.
 */
export function basename(path: string): string {
  const parts = path.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

/** Everything above the last segment, or "" at the root. */
export function dirname(path: string): string {
  const cut = path.lastIndexOf("/");
  return cut > 0 ? path.slice(0, cut) : "";
}
