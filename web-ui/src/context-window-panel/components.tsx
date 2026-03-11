import React, { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { ContextCategory } from "../api";
import { formatTokens, categoryColor } from "./helpers";

/** Individual category row with expandable items */
export function CategoryRow({
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
