import { useState, useCallback, useRef } from "react";
import { GitBranch, ChevronDown, Check, Loader2, RefreshCw } from "lucide-react";
import { useOutsideClick } from "../hooks/useOutsideClick";

interface Props {
  branch: string;
  checkingOut: boolean;
  loading: boolean;
  logLoading: boolean;
  tab: "changes" | "log";
  localBranches: string[];
  remoteBranches: string[];
  branchesLoading: boolean;
  fetchBranchList: () => Promise<void>;
  handleCheckout: (branchName: string) => Promise<void>;
  refreshStatus: () => Promise<void>;
  refreshLog: () => Promise<void>;
}

export function BranchSwitcher({
  branch, checkingOut, loading, logLoading, tab,
  localBranches, remoteBranches, branchesLoading,
  fetchBranchList, handleCheckout,
  refreshStatus, refreshLog,
}: Props) {
  const [showDropdown, setShowDropdown] = useState(false);
  const [filter, setFilter]             = useState("");
  const ref = useRef<HTMLDivElement>(null);

  useOutsideClick(ref, showDropdown, () => { setShowDropdown(false); setFilter(""); });

  const toggle = useCallback(() => {
    setShowDropdown((v) => {
      if (!v) { fetchBranchList(); setFilter(""); }
      return !v;
    });
  }, [fetchBranchList]);

  const onCheckout = useCallback(async (b: string) => {
    await handleCheckout(b);
    setShowDropdown(false);
    setFilter("");
  }, [handleCheckout]);

  const matchFilter = (b: string) => !filter || b.toLowerCase().includes(filter.toLowerCase());
  const filteredLocal  = localBranches.filter(matchFilter);
  const filteredRemote = remoteBranches.filter(matchFilter);

  return (
    <div className="git-panel-branch" ref={ref}>
      <button
        className="git-branch-toggle" onClick={toggle}
        title="Switch branch" aria-label="Switch branch" disabled={checkingOut}
      >
        <GitBranch size={13} />
        <span>{checkingOut ? "Switching..." : branch || "..."}</span>
        <ChevronDown size={10} className={showDropdown ? "rotated" : ""} />
      </button>

      {showDropdown && (
        <div className="git-branch-dropdown">
          <input
            className="git-branch-filter" placeholder="Filter branches..."
            value={filter} onChange={(e) => setFilter(e.target.value)} autoFocus
          />
          {branchesLoading ? (
            <div className="git-branch-loading"><Loader2 size={14} className="spin" /><span>Loading...</span></div>
          ) : (
            <div className="git-branch-list">
              {filteredLocal.map((b) => (
                <button
                  key={b} className={`git-branch-item ${b === branch ? "current" : ""}`}
                  onClick={() => onCheckout(b)} disabled={checkingOut}
                >
                  <span className="git-branch-item-name">{b}</span>
                  {b === branch && <Check size={12} />}
                </button>
              ))}
              {filteredRemote.length > 0 && (
                <>
                  <div className="git-branch-section-label">Remote</div>
                  {filteredRemote.map((b) => (
                    <button
                      key={b} className="git-branch-item remote"
                      onClick={() => onCheckout(b)} disabled={checkingOut}
                    >
                      <span className="git-branch-item-name">{b}</span>
                    </button>
                  ))}
                </>
              )}
              {filteredLocal.length === 0 && filteredRemote.length === 0 && (
                <div className="git-branch-empty">No matching branches</div>
              )}
            </div>
          )}
        </div>
      )}

      <button
        className="git-panel-refresh"
        onClick={() => { if (tab === "changes") refreshStatus(); else refreshLog(); }}
        title="Refresh" aria-label="Refresh"
        disabled={loading || logLoading}
      >
        <RefreshCw size={12} className={loading || logLoading ? "spin" : ""} />
      </button>
    </div>
  );
}
