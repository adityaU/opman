import { useRef } from "react";
import { ChevronLeft, ChevronDown } from "lucide-react";
import type { GitView, GitTab } from "../types";
import { breadcrumbLabel } from "../utils";
import { useOutsideClick } from "../hooks/useOutsideClick";
import { KeyHint } from "../../keybindings/hint/KeyHint";

interface Props {
  viewStack: GitView[];
  tab: GitTab;
  breadcrumbDropdown: boolean;
  setBreadcrumbDropdown: (v: boolean) => void;
  popView: () => void;
  jumpToView: (index: number) => void;
}

export function BreadcrumbNav({ viewStack, tab, breadcrumbDropdown, setBreadcrumbDropdown, popView, jumpToView }: Props) {
  const dropdownRef = useRef<HTMLDivElement>(null);
  useOutsideClick(dropdownRef, breadcrumbDropdown, () => setBreadcrumbDropdown(false));
  const currentView = viewStack[viewStack.length - 1];
  const isFileDiff = currentView?.kind === "file-diff";

  return (
    <>
      {viewStack.length > 1 && (
        <div className="git-breadcrumb-back" ref={dropdownRef}>
          <KeyHint label="Go back" command="git.back"><button className="git-back-btn" onClick={popView} aria-label="Go back"><ChevronLeft size={14} /></button></KeyHint>
          {viewStack.length > 2 && (
            <button className="git-back-dropdown-btn" onClick={() => setBreadcrumbDropdown(!breadcrumbDropdown)} title="Jump to..." aria-label="Jump to previous view"><ChevronDown size={10} /></button>
          )}
          {breadcrumbDropdown && (
            <div className="git-breadcrumb-dropdown">
              {viewStack.slice(0, -1).map((view, index) => (
                <button key={index} className="git-breadcrumb-dropdown-item" onClick={() => jumpToView(index)}>{breadcrumbLabel(view, tab)}</button>
              ))}
            </div>
          )}
        </div>
      )}
      <div className="git-breadcrumb-trail">
        {viewStack.map((view, index) => (
          <span key={index} className="git-breadcrumb-segment">
            {index > 0 && <span className="git-breadcrumb-sep">/</span>}
            {index < viewStack.length - 1 ? (
              <button className="git-breadcrumb-link" onClick={() => jumpToView(index)}>{breadcrumbLabel(view, tab)}</button>
            ) : (
              <span className="git-breadcrumb-current">{breadcrumbLabel(view, tab)}</span>
            )}
          </span>
        ))}
      </div>
      {isFileDiff && (
        <span className={`git-breadcrumb-status ${currentView.staged ? "staged" : "unstaged"}`}>
          {currentView.staged ? "STAGED" : "UNSTAGED"}
        </span>
      )}
    </>
  );
}
