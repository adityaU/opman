/**
 * Shape-matched loading placeholders.
 *
 * A placeholder that has the geometry of what is coming keeps the surface still while
 * data arrives: nothing jumps under the pointer, and the eye already knows where the
 * answer will appear. "Loading…" text and spinners do neither.
 *
 * Motion is a sweep of light across the block (styles/skeleton.css). Under
 * `prefers-reduced-motion` the sweep is removed twice over — by the media query in CSS,
 * and by `data-motion="static"` here, which also makes the choice visible to tests and
 * to any surface that styles placeholders itself.
 */
import type { CSSProperties, ReactNode } from "react";
import { usePrefersReducedMotion } from "../hooks/usePrefersReducedMotion";

type Size = number | string;
const px = (value: Size | undefined) => (typeof value === "number" ? `${value}px` : value);

interface SkeletonProps {
  width?: Size;
  height?: Size;
  /** `pill` for bars and chips, `circle` for avatars and dots, `block` for everything else. */
  shape?: "block" | "pill" | "circle";
  className?: string;
  style?: CSSProperties;
}

/** One placeholder block. Decorative: the surrounding `SkeletonGroup` announces loading. */
export function Skeleton({ width, height = 10, shape = "block", className, style }: SkeletonProps) {
  const reduced = usePrefersReducedMotion();
  return (
    <span
      aria-hidden="true"
      className={`skel skel--${shape}${className ? ` ${className}` : ""}`}
      data-motion={reduced ? "static" : "sweep"}
      style={{ width: px(width), height: px(height), ...style }}
    />
  );
}

interface SkeletonGroupProps {
  /** What is loading, for screen readers: "Loading models". */
  label: string;
  className?: string;
  children: ReactNode;
}

/**
 * The accessible wrapper: one polite status announcement for a cluster of blocks, so a
 * screen reader hears "Loading models" once instead of nothing (or twenty empty spans).
 */
export function SkeletonGroup({ label, className, children }: SkeletonGroupProps) {
  return (
    <div role="status" aria-label={label} aria-busy="true" aria-live="polite" className={`skel-group${className ? ` ${className}` : ""}`}>
      <span className="skel-sr-only">{label}</span>
      {children}
    </div>
  );
}

/** Widths that read like text rather than a ruler. Cycled, never random — no jitter on re-render. */
const LINE_WIDTHS = ["92%", "78%", "86%", "64%", "71%"];

/** A few lines of placeholder text. */
export function SkeletonLines({ lines = 3, height = 10, gap = 8 }: { lines?: number; height?: number; gap?: number }) {
  return (
    <span className="skel-lines" style={{ gap }}>
      {Array.from({ length: lines }, (_, index) => (
        <Skeleton
          key={index}
          height={height}
          width={index === lines - 1 ? "48%" : LINE_WIDTHS[index % LINE_WIDTHS.length]}
        />
      ))}
    </span>
  );
}

/** Name widths for list rows: varied like real names, fixed so nothing jitters. */
const ROW_NAME = [132, 104, 148, 92, 120, 112];
const ROW_DETAIL = [72, 96, 60, 88, 80, 68];

interface SkeletonRowsProps {
  /** What is loading, announced once. */
  label: string;
  count?: number;
  /** The real row's class, so padding and height match what will replace it. */
  rowClassName?: string;
  className?: string;
  /** A leading glyph: an icon square or a colour dot. */
  lead?: "icon" | "dot" | "none";
  /** Description under the name (two-line rows) or beside it (one-line rows). */
  layout?: "inline" | "stacked";
}

/** Placeholder list rows shaped like the rows they stand in for. */
export function SkeletonRows({
  label, count = 4, rowClassName = "", className, lead = "none", layout = "inline",
}: SkeletonRowsProps) {
  return (
    <SkeletonGroup label={label} className={`skel-rows${className ? ` ${className}` : ""}`}>
      {Array.from({ length: count }, (_, index) => {
        const name = ROW_NAME[index % ROW_NAME.length];
        const detail = ROW_DETAIL[index % ROW_DETAIL.length];
        return (
          <div key={index} className={`skel-row skel-row--${layout} ${rowClassName}`.trim()} aria-hidden="true">
            {lead === "icon" && <Skeleton width={12} height={12} />}
            {lead === "dot" && <Skeleton shape="circle" width={7} height={7} />}
            <span className="skel-row-text">
              <Skeleton width={name} height={10} />
              <Skeleton width={layout === "stacked" ? detail * 2 : detail} height={8} />
            </span>
          </div>
        );
      })}
    </SkeletonGroup>
  );
}
