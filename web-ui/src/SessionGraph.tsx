import React, { useState, useEffect, useRef, useCallback } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { GitBranch, Circle, Loader2, X, ChevronRight, ChevronDown } from "lucide-react";
import { fetchSessionsTree } from "./api";
import type { SessionTreeNode, SessionsTreeResponse } from "./api";

interface SessionGraphProps {
  /** Called when user clicks a session node to navigate to it */
  onSelectSession: (projectIndex: number, sessionId: string) => void;
  /** Close button callback */
  onClose: () => void;
  /** Currently active session ID (for highlighting) */
  activeSessionId: string | null;
}

function formatCost(cost: number): string {
  if (cost < 0.01) return `$${cost.toFixed(4)}`;
  return `$${cost.toFixed(2)}`;
}

export function SessionGraph({
  onSelectSession,
  onClose,
  activeSessionId,
}: SessionGraphProps) {
  const [tree, setTree] = useState<SessionsTreeResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  useEffect(() => {
    let cancelled = false;
    fetchSessionsTree()
      .then((data) => {
        if (!cancelled) setTree(data);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const toggleCollapse = useCallback((id: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const handleNodeClick = useCallback(
    (node: SessionTreeNode) => {
      onSelectSession(node.project_index, node.id);
      onClose();
    },
    [onSelectSession, onClose],
  );

  function TreeNode({ node, depth }: { node: SessionTreeNode; depth: number }) {
    const isActive = node.id === activeSessionId;
    const isCollapsed = collapsed.has(node.id);
    const hasChildren = node.children.length > 0;

    return (
      <div className="session-graph-tree-branch" style={{ paddingLeft: depth > 0 ? 16 : 0 }}>
        <button
          className={`session-graph-node${isActive ? " active" : ""}`}
          onClick={() => handleNodeClick(node)}
        >
          {/* Expand/collapse toggle */}
          <span
            className="session-graph-toggle"
            onClick={(e) => {
              if (hasChildren) {
                e.stopPropagation();
                toggleCollapse(node.id);
              }
            }}
          >
            {hasChildren ? (
              isCollapsed ? <ChevronRight size={13} /> : <ChevronDown size={13} />
            ) : (
              <span style={{ width: 13, display: "inline-block" }} />
            )}
          </span>

          <div className="session-graph-node-info">
            <span className="session-graph-node-title">
              {node.title || node.id.slice(0, 8)}
            </span>
            <span className="session-graph-project-tag">{node.project_name}</span>
            {node.stats != null && node.stats.cost > 0 && (
              <span className="session-graph-cost">{formatCost(node.stats.cost)}</span>
            )}
          </div>

          <span className={`session-graph-status ${node.is_busy ? "busy" : "idle"}`}>
            <Circle size={7} />
          </span>
        </button>

        {hasChildren && !isCollapsed && (
          <div className="session-graph-children">
            {node.children.map((child) => (
              <TreeNode key={child.id} node={child} depth={depth + 1} />
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="session-graph-overlay" onClick={onClose}>
      <div
        className="session-graph-panel"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Session Graph"
        ref={modalRef}
      >
        {/* Header */}
        <div className="session-graph-header">
          <GitBranch size={14} />
          <span>Session Graph</span>
          {tree && (
            <span className="session-graph-count">
              {tree.total} session{tree.total !== 1 ? "s" : ""}
            </span>
          )}
          <button
            className="session-graph-close"
            onClick={onClose}
            aria-label="Close session graph"
          >
            <X size={14} />
          </button>
        </div>

        {/* Body */}
        <div className="session-graph-body">
          {loading ? (
            <div className="session-graph-loading">
              <Loader2 size={18} className="spin" />
              <span>Loading session tree…</span>
            </div>
          ) : tree && tree.roots.length > 0 ? (
            tree.roots.map((root) => (
              <TreeNode key={root.id} node={root} depth={0} />
            ))
          ) : (
            <div className="session-graph-empty">No sessions found</div>
          )}
        </div>

        {/* Footer hints */}
        <div className="session-graph-footer">
          <kbd>Esc</kbd> Close
          <span className="session-graph-legend">
            <Circle size={7} className="session-graph-legend-busy" /> busy
            <Circle size={7} className="session-graph-legend-idle" /> idle
          </span>
        </div>
      </div>
    </div>
  );
}
