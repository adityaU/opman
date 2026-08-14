import React from "react";
import { ExternalLink, X } from "lucide-react";

/** Which flavour of request the card carries. Only the accent tint differs. */
export type DockKind = "question" | "permission";

export interface DockTab {
  readonly id: string;
  readonly label: string;
  /** Rendered before the label — the request icon, or an answered marker. */
  readonly icon?: React.ReactNode;
  /** Small uppercase badge, e.g. "sub" for a subagent request. */
  readonly badge?: string;
  /** Question tabs only: the sub-question already has an answer. */
  readonly done?: boolean;
}

interface TabsProps {
  readonly tabs: readonly DockTab[];
  readonly active: number;
  readonly onSelect: (index: number) => void;
  readonly label: string;
  readonly kind: DockKind;
}

/** Tab strip for the pending-request list. Sits above the card, joined to its top edge. */
export const DockTabs = React.memo(function DockTabs({ tabs, active, onSelect, label, kind }: TabsProps) {
  if (tabs.length < 2) return null;
  return (
    <div className={`dock-tabs dock-tabs--${kind}`} role="tablist" aria-label={label}>
      {tabs.map((tab, index) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          className={`dock-tab${index === active ? " dock-tab--active" : ""}${tab.done ? " dock-tab--done" : ""}`}
          aria-selected={index === active}
          onClick={() => onSelect(index)}
        >
          {tab.icon}
          <span className="dock-tab-label">{tab.label}</span>
          {tab.badge && <span className="dock-tab-badge">{tab.badge}</span>}
        </button>
      ))}
    </div>
  );
});

interface CardProps {
  readonly kind: DockKind;
  readonly icon: React.ReactNode;
  readonly title: string;
  readonly subtitle?: string;
  /** Shown as a "subagent" pill next to the title. */
  readonly isCrossSession?: boolean;
  readonly sessionId?: string;
  readonly onGoToSession?: (sessionId: string) => void;
  /** Keyboard hint line, hidden on narrow screens. */
  readonly hint?: string;
  readonly onDismiss?: () => void;
  readonly dismissLabel?: string;
  /** Sub-question tabs, rendered inside the card under the header. */
  readonly tabs?: readonly DockTab[];
  readonly activeTab?: number;
  readonly onSelectTab?: (index: number) => void;
  readonly footer: React.ReactNode;
  readonly cardRef?: React.Ref<HTMLDivElement>;
  readonly onKeyDown?: (event: React.KeyboardEvent) => void;
  readonly children: React.ReactNode;
}

/**
 * The one card shell behind both docks: fixed header, scrollable body, pinned footer.
 *
 * The body is the only part that grows, so a long request can never push the action
 * buttons past the bottom of the viewport.
 */
export const DockCard = React.memo(function DockCard(props: CardProps) {
  const { kind, tabs, activeTab = 0, onSelectTab } = props;
  return (
    <div
      className={`dock-card dock-card--${kind}`}
      ref={props.cardRef}
      tabIndex={-1}
      role="tabpanel"
      onKeyDown={props.onKeyDown}
    >
      <div className="dock-card-head">
        <span className="dock-card-icon">{props.icon}</span>
        <span className="dock-card-heading">
          <span className="dock-card-title">{props.title}</span>
          {props.subtitle && <span className="dock-card-subtitle">{props.subtitle}</span>}
        </span>
        {props.isCrossSession && <span className="dock-card-badge">subagent</span>}
        <span className="dock-card-meta">
          {props.sessionId && props.onGoToSession && (
            <button
              type="button"
              className="dock-session-link"
              onClick={(event) => {
                event.stopPropagation();
                props.onGoToSession?.(props.sessionId as string);
              }}
              title={`Go to session ${props.sessionId.slice(0, 8)}`}
              aria-label="Go to session"
            >
              <ExternalLink size={11} />
              <span>{props.sessionId.slice(0, 8)}</span>
            </button>
          )}
          {props.hint && <span className="dock-card-hint">{props.hint}</span>}
          {props.onDismiss && (
            <button
              type="button"
              className="dock-card-close"
              onClick={props.onDismiss}
              aria-label={props.dismissLabel || "Dismiss"}
              title={props.dismissLabel || "Dismiss"}
            >
              <X size={14} />
            </button>
          )}
        </span>
      </div>
      {tabs && tabs.length > 1 && onSelectTab && (
        <div className="dock-card-tabs" role="tablist" aria-label="Questions">
          {tabs.map((tab, index) => (
            <button
              key={tab.id}
              type="button"
              role="tab"
              className={`dock-subtab${index === activeTab ? " dock-subtab--active" : ""}${tab.done ? " dock-subtab--done" : ""}`}
              aria-selected={index === activeTab}
              onClick={() => onSelectTab(index)}
            >
              <span className="dock-subtab-dot" aria-hidden="true" />
              <span className="dock-tab-label">{tab.label}</span>
            </button>
          ))}
        </div>
      )}
      <div className="dock-card-body">{props.children}</div>
      <div className="dock-card-foot">{props.footer}</div>
    </div>
  );
});
