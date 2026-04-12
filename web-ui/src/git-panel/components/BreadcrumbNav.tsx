import { useRef } from "react";
import { ChevronLeft, ChevronDown } from "lucide-react";
import type { GitView, GitTab } from "../types";
import { breadcrumbLabel } from "../utils";
import { useOutsideClick } from "../hooks/useOutsideClick";

interface Props {
  viewStack: GitView[];
  tab: GitTab;
  breadcrumbDropdown: boolean;
  setBreadcrumbDropdown: (v: boolean) => void;
  popView: () => void;
  jumpToView: (index: number) => void;
}

export function BreadcrumbNav({
  viewStack, tab, breadcrumbDropdown, setBreadcrumbDropdown,
  popView, jumpToView,
}: Props) {
  const dropdownRef = useRef<HTMLDivElement>(null);
  useOutsideClick(dropdownRef, breadcrumbDropdown, () => setBreadcrumbDropdown(false));

  return (
    <>
      {viewStack.length > 1 && (
        <div className="git-breadcrumb-back" ref={dropdownRef}>
          <button className="git-back-btn" onClick={popView} title="Go back" aria-label="Go back">
            <ChevronLeft size={14} />
          </button>
          {viewStack.length > 2 && (
            <button
              className="git-back-dropdown-btn"
              onClick={() => setBreadcrumbDropdown(!breadcrumbDropdown)}
              title="Jump to..." aria-label="Jump to previous view"
            >
              <ChevronDown size={10} />
            </button>
          )}
          {breadcrumbDropdown && (
            <div className="git-breadcrumb-dropdown">
              {viewStack.slice(0, -1).map((v, i) => (
                <button key={i} className="git-breadcrumb-dropdown-item" onClick={() => jumpToView(i)}>
                  {breadcrumbLabel(v, tab)}
                </button>
              ))}
            </div>
          )}
        </div>
      )}

      <div className="git-breadcrumb-trail">
        {viewStack.map((v, i) => (
          <span key={i} className="git-breadcrumb-segment">
            {i > 0 && <span className="git-breadcrumb-sep">/</span>}
            {i < viewStack.length - 1 ? (
              <button className="git-breadcrumb-link" onClick={() => jumpToView(i)}>
                {breadcrumbLabel(v, tab)}
              </button>
            ) : (
              <span className="git-breadcrumb-current">{breadcrumbLabel(v, tab)}</span>
            )}
          </span>
        ))}
      </div>
    </>
  );
}
