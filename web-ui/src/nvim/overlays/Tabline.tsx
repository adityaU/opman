import { useRef, type KeyboardEvent } from "react";
import type { TablineModel } from "../state/tabline";

interface BufferTab {
  readonly id: number | null;
  readonly name: string;
  readonly modified: boolean;
}

type ExtendedTablineModel = TablineModel & {
  readonly curbuf?: unknown;
};

export interface TablineProps {
  readonly state: TablineModel;
  readonly onSelectBuffer?: (buffer: number) => void;
  readonly onSelectTab?: (tab: string) => void;
}

function record(value: unknown): Readonly<Record<string, unknown>> | null {
  return typeof value === "object" && value !== null ? value as Readonly<Record<string, unknown>> : null;
}

function numberValue(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : typeof value === "string" && value.trim() !== "" && Number.isFinite(Number(value)) ? Number(value) : null;
}

function fieldNumber(source: Readonly<Record<string, unknown>>, names: readonly string[]): number | null {
  for (const name of names) {
    const value = numberValue(source[name]);
    if (value !== null) return value;
  }
  return null;
}

function fieldString(source: Readonly<Record<string, unknown>>, names: readonly string[]): string {
  for (const name of names) if (typeof source[name] === "string" && source[name]) return source[name] as string;
  return "";
}

function fieldBoolean(source: Readonly<Record<string, unknown>>, names: readonly string[]): boolean {
  return names.some((name) => source[name] === true);
}

function displayName(name: string): string {
  const value = name.trim();
  if (!value) return "[No Name]";
  const normalized = value.replaceAll("\\", "/");
  const slash = normalized.lastIndexOf("/");
  return normalized.slice(slash + 1) || "[No Name]";
}

function bufferTabs(state: TablineModel): BufferTab[] {
  const extended = state as ExtendedTablineModel;
  const rawBuffers = extended.buffers ?? [];
  const buffers = rawBuffers.map((value): BufferTab => {
    const source = record(value);
    if (!source) return { id: null, name: "[No Name]", modified: false };
    return {
      id: fieldNumber(source, ["buf", "id", "buffer", "number", "bufnr"]),
      name: fieldString(source, ["name", "path", "file"]) || "[No Name]",
      modified: fieldBoolean(source, ["modified", "changed", "dirty"]),
    };
  });
  if (buffers.length > 0) return buffers;
  return state.tabs.map((tab) => {
    const source = record(tab);
    return {
      id: tab.buffer ?? fieldNumber(source ?? {}, ["buf", "id", "buffer", "number", "bufnr"]),
      name: tab.name || fieldString(source ?? {}, ["name", "path", "file"]) || "[No Name]",
      modified: fieldBoolean(source ?? {}, ["modified", "changed", "dirty"]),
    };
  });
}

function currentBuffer(state: TablineModel): number | null {
  const extended = state as ExtendedTablineModel;
  return numberValue(extended.curbuf) ?? numberValue(extended.currentBuffer) ?? numberValue(state.currentTab);
}

function tabTitle(name: string): string {
  return name.trim() || "[No Name]";
}

export function Tabline({ state, onSelectBuffer, onSelectTab }: TablineProps): React.ReactElement | null {
  const bufferButtons = useRef<Array<HTMLButtonElement | null>>([]);
  const buffers = bufferTabs(state);
  if (buffers.length === 0 && state.tabs.length === 0) return null;
  const activeBuffer = currentBuffer(state);
  const activeIndex = buffers.findIndex((buffer) => buffer.id !== null && buffer.id === activeBuffer);
  const focusBuffer = (index: number): void => {
    const next = bufferButtons.current[index];
    if (!next) return;
    next.focus();
    const buffer = buffers[index];
    if (buffer.id !== null) onSelectBuffer?.(buffer.id);
  };
  const onBufferKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number): void => {
    if (buffers.length < 2) return;
    const next = event.key === "ArrowRight" ? (index + 1) % buffers.length : event.key === "ArrowLeft" ? (index - 1 + buffers.length) % buffers.length : event.key === "Home" ? 0 : event.key === "End" ? buffers.length - 1 : -1;
    if (next < 0) return;
    event.preventDefault();
    focusBuffer(next);
  };

  return (
    <nav className="nvim-tabline-overlay" aria-label="Neovim editor navigation" onPointerDown={(event) => event.stopPropagation()}>
      <div className="nvim-buffer-tab-group">
        <div className="nvim-buffer-tab-scroll" role="tablist" aria-label="Open buffers" aria-orientation="horizontal">
          {buffers.map((buffer, index) => {
            const selected = index === activeIndex || (activeIndex < 0 && index === 0);
            const label = displayName(buffer.name);
            const title = tabTitle(buffer.name);
            return (
              <button
                key={`${buffer.id ?? "buffer"}-${index}`}
                ref={(element) => { bufferButtons.current[index] = element; }}
                type="button"
                role="tab"
                tabIndex={selected ? 0 : -1}
                aria-selected={selected}
                aria-label={`${label}${buffer.modified ? ", modified" : ""}`}
                title={title}
                className={`nvim-buffer-tab${selected ? " is-active" : ""}`}
                onClick={() => { if (buffer.id !== null) onSelectBuffer?.(buffer.id); }}
                onKeyDown={(event) => onBufferKeyDown(event, index)}
              >
                <span className="nvim-buffer-tab-label">{label}</span>
                {buffer.modified && <span className="nvim-buffer-tab-modified" aria-label="Modified" />}
              </button>
            );
          })}
        </div>
        {state.tabs.length > 1 && (
          <div className="nvim-tabpage-group" role="group" aria-label="Neovim tab pages">
            {state.tabs.map((tab, index) => {
              const selected = tab.tab === state.currentTab;
              return (
                <button
                  key={tab.tab}
                  type="button"
                  className={`nvim-tabpage-button${selected ? " is-active" : ""}`}
                  aria-pressed={selected}
                  title={`Switch to tab page ${index + 1}`}
                  onClick={() => onSelectTab?.(tab.tab)}
                >
                  {index + 1}
                </button>
              );
            })}
          </div>
        )}
      </div>
    </nav>
  );
}

export default Tabline;
