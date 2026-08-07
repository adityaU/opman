/**
 * MobileBrowserHeader — where you are, and how to get somewhere else.
 *
 * The old header spent its single row on a breadcrumb trail that ran out of
 * width after two levels and a `⋯` button. On a phone the two questions that
 * matter are "which folder am I in" and "where is the file I want", so the
 * folder name gets its own line at a size you can read, the trail sits under it
 * as a scrollable strip, and the search field — which reaches the whole project,
 * not just this folder — is always visible rather than hidden behind a menu.
 */
import { ChevronLeft, Search, X, Loader2, MoreHorizontal } from "lucide-react";
import type { BreadcrumbEntry } from "../../types";

interface Props {
  breadcrumbs: BreadcrumbEntry[];
  query: string;
  searching: boolean;
  hasActions: boolean;
  onQueryChange: (value: string) => void;
  onQueryClear: () => void;
  onNavigate: (path: string) => void;
  onOpenActions: (anchor: DOMRect) => void;
}

export function MobileBrowserHeader({
  breadcrumbs, query, searching, hasActions,
  onQueryChange, onQueryClear, onNavigate, onOpenActions,
}: Props) {
  const current = breadcrumbs[breadcrumbs.length - 1];
  const parent = breadcrumbs[breadcrumbs.length - 2];

  return (
    <div className="xplm-header">
      <div className="xplm-title-row">
        {parent ? (
          <button
            type="button"
            className="xplm-up"
            onClick={() => onNavigate(parent.path)}
            aria-label={`Up to ${parent.label}`}
          >
            <ChevronLeft size={18} />
          </button>
        ) : (
          <span className="xplm-up-spacer" aria-hidden="true" />
        )}

        <span className="xplm-title" title={current?.path}>
          {current?.label ?? "Files"}
        </span>

        {hasActions && (
          <button
            type="button"
            className="xplm-actions-trigger"
            aria-label="Folder actions"
            onClick={(event) => onOpenActions(event.currentTarget.getBoundingClientRect())}
          >
            <MoreHorizontal size={18} />
          </button>
        )}
      </div>

      {breadcrumbs.length > 1 && (
        <div className="xplm-crumbs">
          {breadcrumbs.map((crumb, index) => (
            <button
              key={crumb.path}
              type="button"
              className={`xplm-crumb${index === breadcrumbs.length - 1 ? " is-current" : ""}`}
              onClick={() => onNavigate(crumb.path)}
            >
              {crumb.label}
            </button>
          ))}
        </div>
      )}

      <div className={`xplm-search${query ? " has-query" : ""}`}>
        {searching
          ? <Loader2 size={14} className="xplm-search-icon spin" aria-hidden="true" />
          : <Search size={14} className="xplm-search-icon" aria-hidden="true" />}
        <input
          type="search"
          className="xplm-search-input"
          value={query}
          placeholder="Find a file in this project"
          aria-label="Find a file in this project"
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck={false}
          onChange={(event) => onQueryChange(event.target.value)}
        />
        {query && (
          <button
            type="button"
            className="xplm-search-clear"
            aria-label="Clear search"
            onClick={onQueryClear}
          >
            <X size={13} />
          </button>
        )}
      </div>
    </div>
  );
}
