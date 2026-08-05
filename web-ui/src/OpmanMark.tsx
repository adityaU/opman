/**
 * OpmanMark — the product mark.
 *
 * An open ring (the "O", read as the operations loop) with the agent dot
 * holding the gap: the operator's console with a live agent in orbit. The
 * ring rides `--color-primary` and the dot `--color-accent`, so the mark
 * recolors with every theme palette and works in both glassy and flat modes
 * without variants. `mono` collapses both to `currentColor` for contexts
 * that tint the mark themselves (buttons, tinted badges).
 *
 * Geometry (viewBox 32): ring r=10 centered at 16,16 with a 70° gap centered
 * on the top-right diagonal (−45°); the dot sits on the ring's own radius in
 * the middle of that gap.
 */
export function OpmanMark({
  size = 24,
  mono = false,
  className,
}: {
  size?: number;
  mono?: boolean;
  className?: string;
}) {
  const ring = mono ? "currentColor" : "var(--color-primary)";
  const dot = mono ? "currentColor" : "var(--color-accent)";
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 32 32"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <path
        d="M 25.85 14.26 A 10 10 0 1 1 17.74 6.15"
        stroke={ring}
        strokeWidth="3"
        strokeLinecap="round"
      />
      <circle cx="23.07" cy="8.93" r="2.9" fill={dot} />
    </svg>
  );
}
