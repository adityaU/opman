import React, { useState, useEffect, useMemo, useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Search, Bot, Check } from "lucide-react";
import { fetchAgents, type AgentInfo } from "./api";
import { agentColor } from "./utils/theme";

/**
 * Filter agents the same way opencode does: hide agents with mode "subagent"
 * and those explicitly marked hidden.
 */
function selectableAgents(agents: AgentInfo[]): AgentInfo[] {
  return agents.filter((a) => a.mode !== "subagent" && !a.hidden);
}

interface Props {
  onClose: () => void;
  currentAgent: string;
  onAgentSelected: (agentId: string) => void;
}

export function AgentPickerModal({ onClose, currentAgent, onAgentSelected }: Props) {
  const [allAgents, setAllAgents] = useState<AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  // Fetch agents on mount
  useEffect(() => {
    setLoading(true);
    fetchAgents()
      .then((agents) => {
        setAllAgents(agents);
        setError(null);
      })
      .catch(() => {
        setError("Failed to load agents");
      })
      .finally(() => {
        setLoading(false);
      });
  }, []);

  // Focus the search input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const agents = useMemo(() => {
    const selectable = selectableAgents(allAgents);
    // Sort: current agent first, then alphabetical
    selectable.sort((a, b) => {
      if (a.id === currentAgent && b.id !== currentAgent) return -1;
      if (b.id === currentAgent && a.id !== currentAgent) return 1;
      return a.label.localeCompare(b.label);
    });
    return selectable;
  }, [allAgents, currentAgent]);

  const filtered = useMemo(() => {
    if (!query) return agents;
    const lq = query.toLowerCase();
    return agents.filter(
      (a) =>
        a.id.toLowerCase().includes(lq) ||
        a.label.toLowerCase().includes(lq) ||
        (a.description && a.description.toLowerCase().includes(lq))
    );
  }, [agents, query]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  // Scroll selected item into view
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const item = list.children[selectedIndex] as HTMLElement;
    if (item) item.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  const handleSelect = (agent: AgentInfo) => {
    onAgentSelected(agent.id);
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (filtered[selectedIndex]) {
        handleSelect(filtered[selectedIndex]);
      }
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="agent-picker"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Choose agent"
        ref={modalRef}
      >
        <div className="agent-picker-header">
          <Bot size={16} />
          <span>Choose Agent</span>
          <span className="agent-picker-count">
            {filtered.length} agent{filtered.length !== 1 ? "s" : ""}
          </span>
        </div>
        <div className="agent-picker-input-row">
          <Search size={14} />
          <input
            ref={inputRef}
            className="agent-picker-input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search agents..."
          />
        </div>
        <div className="agent-picker-results" ref={listRef}>
          {loading ? (
            <div className="agent-picker-empty">Loading agents...</div>
          ) : error ? (
            <div className="agent-picker-empty agent-picker-error">{error}</div>
          ) : filtered.length === 0 ? (
            <div className="agent-picker-empty">No agents found</div>
          ) : (
            filtered.map((agent, idx) => {
              const color = agentColor(agent.id, agent.color);
              const isCurrent = agent.id === currentAgent;
              return (
                <button
                  key={agent.id}
                  className={`agent-picker-item ${idx === selectedIndex ? "selected" : ""}`}
                  onClick={() => handleSelect(agent)}
                  onMouseEnter={() => setSelectedIndex(idx)}
                >
                  <div className="agent-picker-item-left">
                    <span className="agent-picker-name">
                      {isCurrent && <Check size={10} className="agent-current-icon" />}
                      {color && (
                        <span
                          className="agent-picker-dot"
                          style={{ backgroundColor: color }}
                        />
                      )}
                      {agent.label}
                    </span>
                    <span className="agent-picker-desc">
                      {agent.description}
                      {agent.id !== agent.label.toLowerCase() && (
                        <span className="agent-picker-id"> &middot; {agent.id}</span>
                      )}
                    </span>
                  </div>
                  <div className="agent-picker-item-right">
                    {agent.mode && agent.mode !== "primary" && (
                      <span className="agent-picker-mode">{agent.mode}</span>
                    )}
                    {agent.native && (
                      <span className="agent-picker-native">built-in</span>
                    )}
                  </div>
                </button>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
