import type { PopupmenuModel } from "../state/popupmenu";

export interface PopupmenuProps {
  readonly state: PopupmenuModel;
  readonly cellWidth: number;
  readonly cellHeight: number;
}

function kindLabel(kind: string): string {
  const value = kind.trim();
  return value ? value.slice(0, 3).toUpperCase() : "ITEM";
}

export function Popupmenu({ state, cellWidth, cellHeight }: PopupmenuProps): React.ReactElement | null {
  if (!state.visible) return null;
  const selectedItem = state.items[state.selected];
  const activeId = selectedItem ? `nvim-popup-${state.selected}` : undefined;
  const left = `min(calc(var(--panel-inset) + ${Math.max(0, state.col) * cellWidth}px), calc(100% - var(--panel-inset) - min(38rem, calc(100% - var(--panel-inset) * 2))))`;
  const top = `min(calc(var(--panel-inset) + ${Math.max(0, state.row) * cellHeight}px), calc(100% - var(--panel-inset) - var(--pane-head-h)))`;
  return (
    <div className="nvim-popupmenu-overlay modal-popover-surface" style={{ left, top }}>
      <div
        className="nvim-popupmenu-list cm-tooltip-autocomplete"
        role="listbox"
        tabIndex={-1}
        aria-label="Neovim completions"
        aria-activedescendant={activeId}
      >
        {state.items.length === 0 ? (
          <div className="nvim-popupmenu-empty">No completions</div>
        ) : state.items.map((item, index) => {
          const selected = index === state.selected;
          const label = item.abbr || item.word || "Unnamed completion";
          return (
            <div
              id={`nvim-popup-${index}`}
              className={`nvim-popupmenu-item${selected ? " is-selected" : ""}`}
              key={`${item.word}-${index}`}
              role="option"
              aria-selected={selected}
              aria-label={`${item.word || label}${item.menu ? `, ${item.menu}` : ""}`}
              title={item.info || item.word || undefined}
            >
              <span className="nvim-popupmenu-kind" aria-hidden="true">{kindLabel(item.kind)}</span>
              <span className="nvim-popupmenu-word">{label}</span>
              {item.menu && <span className="nvim-popupmenu-menu">{item.menu}</span>}
            </div>
          );
        })}
      </div>
      {selectedItem?.info && <div className="nvim-popupmenu-info">{selectedItem.info}</div>}
    </div>
  );
}

export default Popupmenu;
