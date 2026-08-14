/**
 * The panel's five sections as a segmented tablist.
 *
 * At ~280px five labelled tabs cannot fit, so the labels are dropped by the
 * stylesheet and the accessible name moves to `aria-label`/`title` — the tab
 * stays operable and announceable at every width, it just stops spelling
 * itself out.
 */

import { useRef } from "react";
import { Archive, FileDiff, GitBranch, History, TreeDeciduous } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";

import type { GitSection } from "../types";

export interface SectionCounts {
  changes: number;
  branches: number;
  worktrees: number;
  stashes: number;
}

export interface SectionNavProps {
  section: GitSection;
  onChange: (section: GitSection) => void;
  counts: SectionCounts;
}

interface Tab {
  id: GitSection;
  label: string;
  Icon: LucideIcon;
  /** History has no meaningful count — a commit total is not a to-do list. */
  count?: keyof SectionCounts;
}

const TABS: Tab[] = [
  { id: "changes", label: "Changes", Icon: FileDiff, count: "changes" },
  { id: "history", label: "History", Icon: History },
  { id: "branches", label: "Branches", Icon: GitBranch, count: "branches" },
  { id: "worktrees", label: "Worktrees", Icon: TreeDeciduous, count: "worktrees" },
  { id: "stashes", label: "Stashes", Icon: Archive, count: "stashes" },
];

export function SectionNav({ section, onChange, counts }: SectionNavProps) {
  const tabs = useRef<Array<HTMLButtonElement | null>>([]);

  const move = (from: number, delta: number) => {
    const next = (from + delta + TABS.length) % TABS.length;
    onChange(TABS[next].id);
    tabs.current[next]?.focus();
  };

  const onKeyDown = (event: ReactKeyboardEvent<HTMLButtonElement>, index: number) => {
    switch (event.key) {
      case "ArrowRight":
      case "ArrowDown":
        event.preventDefault();
        move(index, 1);
        break;
      case "ArrowLeft":
      case "ArrowUp":
        event.preventDefault();
        move(index, -1);
        break;
      case "Home":
        event.preventDefault();
        move(0, 0);
        break;
      case "End":
        event.preventDefault();
        move(TABS.length - 1, 0);
        break;
      default:
        break;
    }
  };

  return (
    <div className="gitp-nav" role="tablist" aria-label="Git sections">
      {TABS.map((tab, index) => {
        const selected = tab.id === section;
        const count = tab.count ? counts[tab.count] : 0;
        const name = count > 0 ? `${tab.label}, ${count}` : tab.label;
        return (
          <button
            key={tab.id}
            ref={(node) => {
              tabs.current[index] = node;
            }}
            type="button"
            role="tab"
            id={`gitp-tab-${tab.id}`}
            className="gitp-nav-tab"
            data-selected={selected ? "" : undefined}
            aria-selected={selected}
            aria-label={name}
            title={name}
            tabIndex={selected ? 0 : -1}
            onKeyDown={(event) => onKeyDown(event, index)}
            onClick={() => onChange(tab.id)}
          >
            <tab.Icon className="gitp-icon" aria-hidden={true} />
            <span className="gitp-nav-label">{tab.label}</span>
            {count > 0 ? <span className="gitp-nav-count">{count}</span> : null}
          </button>
        );
      })}
    </div>
  );
}
