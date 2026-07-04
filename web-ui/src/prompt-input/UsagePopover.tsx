import React, { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Info } from "lucide-react";
import type { SessionStats } from "../api";
import { formatTokens, categoryColor } from "../context-window-panel/helpers";
import { computeUsageBreakdown, formatCost } from "./usageCost";

interface Anchor {
  /** Distance from the viewport's right edge. */
  right: number;
  /** Distance from the viewport's bottom edge. */
  bottom: number;
}

interface UsagePopoverProps {
  breakdown: ReturnType<typeof computeUsageBreakdown>;
  anchor: Anchor;
  popoverRef: React.RefObject<HTMLDivElement>;
}

/** Popover body: stacked bar + per-category rows + total cost.
 *  Rendered via portal with fixed positioning so it escapes the prompt
 *  input wrapper's `overflow: hidden` (used for its glass-edge clipping). */
function UsagePopover({ breakdown, anchor, popoverRef }: UsagePopoverProps) {
  const { rows, totalTokens, totalCost } = breakdown;

  return (
    <div className="usage-popover" ref={popoverRef} role="dialog" aria-label="Session usage and cost"
      style={{ position: "fixed", right: anchor.right, bottom: anchor.bottom }}>
      <div className="usage-popover-header">
        <span>Session Usage</span>
        <span className="usage-popover-total-cost">{formatCost(totalCost)}</span>
      </div>

      <div className="ctx-stacked-bar">
        {rows.map((row) => (
          <div key={row.key} className="ctx-stacked-segment"
            style={{ width: `${Math.max(row.pct, 0.5)}%`, backgroundColor: categoryColor(row.color) }}
            title={`${row.label}: ${formatTokens(row.tokens)} (${row.pct.toFixed(1)}%)`} />
        ))}
      </div>

      <div className="usage-popover-rows">
        {rows.map((row) => (
          <div key={row.key} className="usage-popover-row">
            <div className="usage-popover-row-top">
              <span className="ctx-category-dot" style={{ backgroundColor: categoryColor(row.color) }} />
              <span className="usage-popover-row-label">{row.label}</span>
              <span className="usage-popover-row-tokens">{formatTokens(row.tokens)}</span>
              <span className="usage-popover-row-pct">{row.pct.toFixed(1)}%</span>
              <span className="usage-popover-row-cost">{formatCost(row.cost)}</span>
            </div>
            <div className="ctx-category-bar-track">
              <div className="ctx-category-bar-fill"
                style={{ width: `${Math.min(row.pct, 100)}%`, backgroundColor: categoryColor(row.color) }} />
            </div>
          </div>
        ))}
      </div>

      <div className="usage-popover-footer">
        <span>{formatTokens(totalTokens)} tokens</span>
        <span>{formatCost(totalCost)}</span>
      </div>
      <div className="usage-popover-note">
        Cost is split across categories using standard relative token pricing; the total always matches your session's actual cost.
      </div>
    </div>
  );
}

interface UsageInfoButtonProps {
  stats: SessionStats | null | undefined;
}

/** "i" info button for the pill row — opens a token/cost breakdown popover. */
export function UsageInfoButton({ stats }: UsageInfoButtonProps) {
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<Anchor | null>(null);
  const btnRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (btnRef.current?.contains(target) || popoverRef.current?.contains(target)) return;
      setOpen(false);
    };
    const reposition = () => {
      if (!btnRef.current) return;
      const rect = btnRef.current.getBoundingClientRect();
      setAnchor({ right: window.innerWidth - rect.right, bottom: window.innerHeight - rect.top + 6 });
    };
    document.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("resize", reposition);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("resize", reposition);
    };
  }, [open]);

  if (!stats) return null;
  const breakdown = computeUsageBreakdown(stats);
  if (breakdown.totalTokens === 0) return null;

  const handleToggle = () => {
    if (!open && btnRef.current) {
      const rect = btnRef.current.getBoundingClientRect();
      setAnchor({ right: window.innerWidth - rect.right, bottom: window.innerHeight - rect.top + 6 });
    }
    setOpen((v) => !v);
  };

  return (
    <div className="usage-info-wrap">
      <button ref={btnRef} className="usage-info-btn" onClick={handleToggle}
        title="Session usage & cost" aria-label="Session usage and cost breakdown">
        <Info size={12} />
      </button>
      {open && anchor && createPortal(
        <UsagePopover breakdown={breakdown} anchor={anchor} popoverRef={popoverRef} />,
        document.body,
      )}
    </div>
  );
}
