import { History } from "lucide-react";
import type { GitTab } from "../types";

interface Props {
  tab: GitTab;
  setTab: (t: GitTab) => void;
  totalChanges: number;
  resetStack: () => void;
}

export function GitTabBar({ tab, setTab, totalChanges, resetStack }: Props) {
  return (
    <div className="git-panel-tabs">
      <button className={`git-tab ${tab === "changes" ? "active" : ""}`} onClick={() => setTab("changes")}>
        Changes
        {totalChanges > 0 && <span className="git-tab-badge">{totalChanges}</span>}
      </button>
      <button
        className={`git-tab ${tab === "log" ? "active" : ""}`}
        onClick={() => { setTab("log"); resetStack(); }}
      >
        <History size={12} />
        Log
      </button>
    </div>
  );
}
