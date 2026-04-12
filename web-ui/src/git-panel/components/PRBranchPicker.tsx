import { useState, useMemo } from "react";
import { X, GitPullRequest, Search, Loader2, ChevronRight } from "lucide-react";

interface Props {
  currentBranch: string;
  localBranches: string[];
  remoteBranches: string[];
  loading: boolean;
  onSelect: (baseBranch: string) => void;
  onClose: () => void;
}

/** Common default base branches, in priority order. */
const DEFAULT_BASES = ["main", "master", "develop", "dev"];

export function PRBranchPicker({
  currentBranch, localBranches, remoteBranches, loading, onSelect, onClose,
}: Props) {
  const [filter, setFilter] = useState("");

  // Deduplicate & exclude the current branch; combine local + cleaned remote names
  const allBranches = useMemo(() => {
    const remoteClean = remoteBranches
      .map((b) => b.replace(/^origin\//, ""))
      .filter((b) => b !== "HEAD");
    const set = new Set([...localBranches, ...remoteClean]);
    set.delete(currentBranch);
    return Array.from(set).sort();
  }, [localBranches, remoteBranches, currentBranch]);

  // Pick a sensible default suggestion
  const suggestedBase = useMemo(() => {
    for (const d of DEFAULT_BASES) {
      if (allBranches.includes(d)) return d;
    }
    return allBranches[0] ?? "";
  }, [allBranches]);

  const filtered = useMemo(() => {
    if (!filter) return allBranches;
    const lower = filter.toLowerCase();
    return allBranches.filter((b) => b.toLowerCase().includes(lower));
  }, [allBranches, filter]);

  return (
    <div className="pr-branch-picker-overlay" onClick={onClose}>
      <div className="pr-branch-picker" onClick={(e) => e.stopPropagation()}>
        <div className="pr-branch-picker-header">
          <GitPullRequest size={14} />
          <span>
            Draft PR: {currentBranch} <ChevronRight size={12} /> <em>select target branch</em>
          </span>
          <button className="pr-branch-picker-close" onClick={onClose}><X size={14} /></button>
        </div>

        <div className="pr-branch-picker-search">
          <Search size={12} />
          <input
            type="text"
            placeholder="Filter branches..."
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && filtered.length > 0 && !loading) {
                e.preventDefault();
                // Select the suggested base if it's in the filtered list, otherwise the first match
                const target = filtered.includes(suggestedBase) ? suggestedBase : filtered[0];
                onSelect(target);
              }
            }}
            autoFocus
          />
        </div>

        <div className="pr-branch-picker-list">
          {loading && filtered.length === 0 ? (
            <div className="pr-branch-picker-empty">
              <Loader2 size={14} className="spin" /> Loading branches...
            </div>
          ) : filtered.length === 0 ? (
            <div className="pr-branch-picker-empty">No matching branches</div>
          ) : (
            filtered.map((b) => (
              <button
                key={b}
                className={`pr-branch-picker-item${b === suggestedBase ? " suggested" : ""}`}
                onClick={() => onSelect(b)}
                disabled={loading}
              >
                <span className="pr-branch-picker-item-name">{b}</span>
                {b === suggestedBase && (
                  <span className="pr-branch-picker-item-badge">default</span>
                )}
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
