/** Format a token count for display */
export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

/** Get CSS color variable for a category */
export function categoryColor(color: string): string {
  switch (color) {
    case "blue":
      return "var(--color-info)";
    case "green":
      return "var(--color-success)";
    case "orange":
      return "var(--color-warning)";
    case "purple":
      return "var(--color-accent)";
    case "gray":
      return "var(--color-text-muted)";
    default:
      return "var(--color-text-muted)";
  }
}
