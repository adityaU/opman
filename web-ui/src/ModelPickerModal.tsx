import React, { useState, useEffect, useMemo, useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useProviders } from "./hooks/useProviders";
import { Search, Cpu, Check, RefreshCw } from "lucide-react";

interface Props {
  onClose: () => void;
  sessionId: string | null;
  onModelSelected?: (modelId: string, providerId: string) => void;
}

interface FlatModel {
  providerId: string;
  providerName: string;
  modelId: string;
  modelName: string;
  contextWindow?: number;
  isConnected: boolean;
  isDefault: boolean;
}

export function ModelPickerModal({ onClose, sessionId, onModelSelected }: Props) {
  const providers = useProviders();
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [showAll, setShowAll] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const flatModels = useMemo<FlatModel[]>(() => {
    const result: FlatModel[] = [];
    const providerFilter = showAll
      ? providers.all
      : providers.all.filter((p) => providers.connected.has(p.id));

    for (const p of providerFilter) {
      const provName = p.name || p.id;
      if (p.models) {
        for (const [modelId, modelInfo] of Object.entries(p.models)) {
          const name = modelInfo.name || modelId;
          result.push({
            providerId: p.id,
            providerName: provName,
            modelId,
            modelName: name,
            contextWindow: modelInfo.limit?.context,
            isConnected: providers.connected.has(p.id),
            isDefault: providers.defaults[p.id] === modelId,
          });
        }
      }
    }

    // Sort: default models first, then alphabetical
    result.sort((a, b) => {
      if (a.isDefault !== b.isDefault) return a.isDefault ? -1 : 1;
      return a.modelName.localeCompare(b.modelName);
    });

    return result;
  }, [providers.all, providers.connected, providers.defaults, showAll]);

  const filtered = useMemo(() => {
    if (!query) return flatModels;
    const lq = query.toLowerCase();
    return flatModels.filter(
      (m) =>
        m.modelId.toLowerCase().includes(lq) ||
        m.modelName.toLowerCase().includes(lq) ||
        m.providerName.toLowerCase().includes(lq) ||
        m.providerId.toLowerCase().includes(lq)
    );
  }, [flatModels, query]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query, showAll]);

  // Scroll selected item into view
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const item = list.children[selectedIndex] as HTMLElement;
    if (item) item.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  const handleSelect = (model: FlatModel) => {
    if (!sessionId) return;
    // Don't call the broken command endpoint — just set the model locally.
    // The model will be sent with each message via the `model` field.
    onModelSelected?.(model.modelId, model.providerId);
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
    } else if (e.key === "Tab") {
      e.preventDefault();
      setShowAll((v) => !v);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="model-picker" onClick={(e) => e.stopPropagation()}>
        <div className="model-picker-header">
          <Cpu size={16} />
          <span>Choose Model</span>
          <span className="model-picker-count">
            {filtered.length} model{filtered.length !== 1 ? "s" : ""}
          </span>
        </div>
        <div className="model-picker-input-row">
          <Search size={14} />
          <input
            ref={inputRef}
            className="model-picker-input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search models..."
          />
        </div>
        <div className="model-picker-tabs">
          <button
            className={`model-picker-tab ${!showAll ? "active" : ""}`}
            onClick={() => setShowAll(false)}
          >
            Connected ({providers.connected.size})
          </button>
          <button
            className={`model-picker-tab ${showAll ? "active" : ""}`}
            onClick={() => setShowAll(true)}
          >
            All Providers ({providers.all.length})
          </button>
          <button
            className="model-picker-refresh"
            onClick={providers.refresh}
            disabled={providers.loading}
            title="Refresh providers"
          >
            <RefreshCw size={12} className={providers.loading ? "spinning" : ""} />
          </button>
          <span className="model-picker-tab-hint">Tab to switch</span>
        </div>
        <div className="model-picker-results" ref={listRef}>
          {providers.loading ? (
            <div className="model-picker-empty">Loading providers...</div>
          ) : providers.error ? (
            <div className="model-picker-empty model-picker-error">
              {providers.error}
            </div>
          ) : filtered.length === 0 ? (
            <div className="model-picker-empty">
              {showAll ? "No models found" : "No connected providers found. Press Tab to show all."}
            </div>
          ) : (
            filtered.map((model, idx) => (
              <button
                key={`${model.providerId}-${model.modelId}`}
                className={`model-picker-item ${idx === selectedIndex ? "selected" : ""}`}
                onClick={() => handleSelect(model)}
                onMouseEnter={() => setSelectedIndex(idx)}
              >
                <div className="model-picker-item-left">
                  <span className="model-picker-name">
                    {model.isDefault && <Check size={10} className="model-default-icon" />}
                    {model.modelName}
                  </span>
                  <span className="model-picker-provider">
                    {model.providerName}
                  </span>
                </div>
                {model.contextWindow && (
                  <span className="model-picker-ctx">
                    {Math.round(model.contextWindow / 1000)}K ctx
                  </span>
                )}
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
