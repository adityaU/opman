/**
 * Layout density: how much air sits between every surface in the shell.
 *
 * There is exactly one number behind this. `--panel-inset` is the distance
 * from the shell to the viewport on all four sides, the gap between the
 * sidebar and the workspace, the gap between two panes, and the gap between
 * the pane tree and the window rail — they were five different values until
 * they were collapsed onto this token, which is what makes a single control
 * able to move all of them together.
 *
 * Stored and applied exactly like the appearance preference in ./appearance:
 * a localStorage string, read once at boot, written as an inline custom
 * property on <html> so it beats the stylesheet's :root default.
 */

export type Density = "compact" | "default" | "roomy";

const DENSITY_KEY = "opman-density";

/** The inset each step buys. `default` matches panel-layout.css's :root. */
const INSET: Readonly<Record<Density, string>> = {
  compact: "6px",
  default: "12px",
  roomy: "20px",
};

function isDensity(value: string | null): value is Density {
  return value === "compact" || value === "default" || value === "roomy";
}

export function loadDensity(): Density {
  try {
    const stored = localStorage.getItem(DENSITY_KEY);
    return isDensity(stored) ? stored : "default";
  } catch {
    return "default";
  }
}

export function persistDensity(density: Density): void {
  try {
    localStorage.setItem(DENSITY_KEY, density);
  } catch {
    /* private mode — the session still applies, it just will not survive */
  }
}

export function applyDensity(density: Density): void {
  document.documentElement.style.setProperty("--panel-inset", INSET[density]);
  document.documentElement.setAttribute("data-density", density);
}

/** Read the stored preference and apply it. Returns what it applied. */
export function initDensity(): Density {
  const density = loadDensity();
  applyDensity(density);
  return density;
}
