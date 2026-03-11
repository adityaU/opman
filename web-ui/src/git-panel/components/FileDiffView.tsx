import ReactDiffViewer, { DiffMethod } from "react-diff-viewer-continued";
import { FileText, Loader2 } from "lucide-react";
import type { GitView } from "../types";

interface Props {
  currentView: GitView & { kind: "file-diff" };
  diffOld: string;
  diffNew: string;
  diffLoading: boolean;
  diffStyles: ReturnType<typeof import("../utils").buildDiffStyles>;
}

export function FileDiffView({ currentView, diffOld, diffNew, diffLoading, diffStyles }: Props) {
  return (
    <div className="git-diff-fullview">
      <div className="git-diff-header">
        <FileText size={12} />
        <span>{currentView.file}</span>
        <span className="git-diff-type">{currentView.staged ? "staged" : "unstaged"}</span>
      </div>
      <div className="git-diff-body">
        {diffLoading ? (
          <div className="git-loading"><Loader2 size={18} className="spin" /></div>
        ) : diffOld === "" && diffNew === "" ? (
          <div className="git-empty"><span>No diff available</span></div>
        ) : (
          <ReactDiffViewer
            oldValue={diffOld} newValue={diffNew}
            splitView={false} useDarkTheme={true}
            compareMethod={DiffMethod.LINES} styles={diffStyles}
          />
        )}
      </div>
    </div>
  );
}
