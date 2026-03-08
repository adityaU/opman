import React, { useState, useEffect, useRef, useCallback } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { fetchContextWindow, executeCommand } from "./api";
import type { ContextWindowResponse, ContextCategory } from "./api";
import {
  X,
  Loader2,
  Zap,
  ChevronDown,
  ChevronRight,
  Minimize2,
  MessageSquare,
  RefreshCw,
} from "lucide-react";

interface Props {
  onClose: () => void;
  sessionId: string | null;
  /** Callback when compact is triggered (to show toast) */
  onCompact?: () => void;
}

/** Format a token count for display */
function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

/** Get CSS color variable for a category */
function categoryColor(color: string): string {
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

/** Individual category row with expandable items */
function CategoryRow({
  category,
  contextLimit,
}: {
  category: ContextCategory;
  contextLimit: number;
}) {
  const [expanded, setExpanded] = useState(false);
  const barWidth = contextLimit > 0 ? (category.tokens / contextLimit) * 100 : 0;

  return (
    <div className="ctx-category">
      <button
        className="ctx-category-header"
        onClick={() => category.items.length > 0 && setExpanded((v) => !v)}
        aria-expanded={expanded}
      >
        <span className="ctx-category-expand">
          {category.items.length > 0 ? (
            expanded ? (
              <ChevronDown size={12} />
            ) : (
              <ChevronRight size={12} />
            )
          ) : (
            <span style={{ width: 12 }} />
          )}
        </span>
        <span
          className="ctx-category-dot"
          style={{ backgroundColor: categoryColor(category.color) }}
        />
        <span className="ctx-category-label">{category.label}</span>
        <span className="ctx-category-tokens">
          {formatTokens(category.tokens)}
        </span>
        <span className="ctx-category-pct">{category.pct.toFixed(1)}%</span>
      </button>
      {/* Mini bar */}
      <div className="ctx-category-bar-track">
        <div
          className="ctx-category-bar-fill"
          style={{
            width: `${Math.min(barWidth, 100)}%`,
            backgroundColor: categoryColor(category.color),
          }}
        />
      </div>
      {/* Expanded items */}
      {expanded && category.items.length > 0 && (
        <div className="ctx-category-items">
          {category.items.map((item, i) => (
            <div key={i} className="ctx-item">
              <span className="ctx-item-label">{item.label}</span>
              <span className="ctx-item-tokens">
                {formatTokens(item.tokens)}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function ContextWindowPanel({
  onClose,
  sessionId,
  onCompact,
}: Props) {
  const [data, setData] = useState<ContextWindowResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [compacting, setCompacting] = useState(false);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  const loadData = useCallback(() => {
    setLoading(true);
    setError(null);
    fetchContextWindow(sessionId ?? undefined)
      .then((result) => {
        setData(result);
        setLoading(false);
      })
      .catch(() => {
        setError("Failed to load context window data");
        setLoading(false);
      });
  }, [sessionId]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleCompact = useCallback(async () => {
    if (!sessionId || compacting) return;
    setCompacting(true);
    try {
      await executeCommand(sessionId, "compact");
      onCompact?.();
      // Reload after compact
      setTimeout(loadData, 1000);
    } catch {
      // Ignore
    } finally {
      setCompacting(false);
    }
  }, [sessionId, compacting, onCompact, loadData]);

  // Gauge color
  const gaugeColor =
    data && data.usage_pct > 90
      ? "var(--color-error)"
      : data && data.usage_pct > 70
        ? "var(--color-warning)"
        : "var(--color-success)";

  // Gauge ring
  const radius = 54;
  const circumference = 2 * Math.PI * radius;
  const usageFraction = data ? Math.min(data.usage_pct / 100, 1) : 0;
  const strokeDashoffset = circumference * (1 - usageFraction);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="ctx-window-modal"
        onClick={(e) => e.stopPropagation()}
        tabIndex={0}
        role="dialog"
        aria-modal="true"
        aria-label="Context window usage"
        ref={modalRef}
      >
        {/* Header */}
        <div className="ctx-window-header">
          <Zap size={14} />
          <span>Context Window</span>
          <div className="ctx-window-header-actions">
            <button
              className="ctx-window-refresh"
              onClick={loadData}
              title="Refresh"
              aria-label="Refresh context data"
            >
              <RefreshCw size={13} className={loading ? "spinning" : ""} />
            </button>
            <button
              className="ctx-window-close"
              onClick={onClose}
              aria-label="Close"
            >
              <X size={14} />
            </button>
          </div>
        </div>

        {loading ? (
          <div className="ctx-window-loading">
            <Loader2 size={18} className="spinning" />
            <span>Loading context data...</span>
          </div>
        ) : error || !data ? (
          <div className="ctx-window-error">{error || "No data available"}</div>
        ) : (
          <>
            {/* Gauge + summary */}
            <div className="ctx-window-gauge-section">
              <div className="ctx-window-gauge">
                <svg viewBox="0 0 128 128" className="ctx-gauge-svg">
                  {/* Background ring */}
                  <circle
                    cx="64"
                    cy="64"
                    r={radius}
                    fill="none"
                    stroke="var(--color-border)"
                    strokeWidth="8"
                  />
                  {/* Usage ring */}
                  <circle
                    cx="64"
                    cy="64"
                    r={radius}
                    fill="none"
                    stroke={gaugeColor}
                    strokeWidth="8"
                    strokeDasharray={circumference}
                    strokeDashoffset={strokeDashoffset}
                    strokeLinecap="round"
                    transform="rotate(-90 64 64)"
                    className="ctx-gauge-ring"
                  />
                  {/* Center text */}
                  <text
                    x="64"
                    y="58"
                    textAnchor="middle"
                    className="ctx-gauge-pct"
                    fill={gaugeColor}
                  >
                    {Math.round(data.usage_pct)}%
                  </text>
                  <text
                    x="64"
                    y="76"
                    textAnchor="middle"
                    className="ctx-gauge-label"
                    fill="var(--color-text-muted)"
                  >
                    used
                  </text>
                </svg>
              </div>
              <div className="ctx-window-summary">
                <div className="ctx-breakdown-title">Session Budget</div>
                <div className="ctx-summary-row">
                  <span className="ctx-summary-label">Used</span>
                  <span className="ctx-summary-value">
                    {formatTokens(data.total_used)}
                  </span>
                </div>
                <div className="ctx-summary-row">
                  <span className="ctx-summary-label">Limit</span>
                  <span className="ctx-summary-value">
                    {formatTokens(data.context_limit)}
                  </span>
                </div>
                <div className="ctx-summary-row">
                  <span className="ctx-summary-label">Remaining</span>
                  <span className="ctx-summary-value">
                    {formatTokens(
                      Math.max(0, data.context_limit - data.total_used)
                    )}
                  </span>
                </div>
                {data.estimated_messages_remaining !== null && (
                  <div className="ctx-summary-row ctx-summary-estimate">
                    <MessageSquare size={11} />
                    <span>
                      ~{data.estimated_messages_remaining} messages remaining
                    </span>
                  </div>
                )}
              </div>
            </div>

            {/* Category breakdown */}
            <div className="ctx-window-breakdown">
              <div className="ctx-breakdown-title">Token Breakdown</div>
              {/* Stacked bar */}
              <div className="ctx-stacked-bar">
                {data.categories.map((cat) => {
                  const w =
                    data.context_limit > 0
                      ? (cat.tokens / data.context_limit) * 100
                      : 0;
                  return (
                    <div
                      key={cat.name}
                      className="ctx-stacked-segment"
                      style={{
                        width: `${Math.max(w, 0.5)}%`,
                        backgroundColor: categoryColor(cat.color),
                      }}
                      title={`${cat.label}: ${formatTokens(cat.tokens)} (${cat.pct.toFixed(1)}%)`}
                    />
                  );
                })}
                {/* Remaining */}
                {data.total_used < data.context_limit && (
                  <div
                    className="ctx-stacked-segment ctx-stacked-remaining"
                    style={{
                      width: `${((data.context_limit - data.total_used) / data.context_limit) * 100}%`,
                    }}
                  />
                )}
              </div>
              {/* Category rows */}
              {data.categories.map((cat) => (
                <CategoryRow
                  key={cat.name}
                  category={cat}
                  contextLimit={data.context_limit}
                />
              ))}
            </div>

            {/* Actions */}
            {sessionId && (
              <div className="ctx-window-actions">
                <button
                  className="ctx-action-btn"
                  onClick={handleCompact}
                  disabled={compacting}
                  title="Compact conversation to free context space"
                >
                  {compacting ? (
                    <Loader2 size={13} className="spinning" />
                  ) : (
                    <Minimize2 size={13} />
                  )}
                  <span>Compact History</span>
                </button>
              </div>
            )}
          </>
        )}

        {/* Footer */}
        <div className="ctx-window-footer">
          <kbd>Esc</kbd> Close
          <kbd>R</kbd> Refresh
        </div>
      </div>
    </div>
  );
}
