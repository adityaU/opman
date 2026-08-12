import { EditorView, showPanel, type Panel } from "@codemirror/view";
import { bindingStateField, type BindingSnapshot } from "./decorations";
import type { NvimLayout } from "./wire";

/** Neovim's own tab line is redundant while there is one window and one file;
 *  it appears once splits, tab pages or unsaved buffers make it informative. */
function isInformative(layout: NvimLayout): boolean {
  return layout.tabpages > 1 || layout.windows > 1 || layout.buffers.some((entry) => entry.modified);
}

function element(tag: string, className: string, text?: string): HTMLElement {
  const node = document.createElement(tag);
  node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

function tabpageGroup(layout: NvimLayout): HTMLElement {
  const group = element("div", "nvim-tabpage-group");
  group.setAttribute("aria-label", "Neovim windows and tab pages");
  group.append(
    element("span", "nvim-tabpage-count", plural(layout.tabpages, "tab")),
    element("span", "nvim-tabpage-windows", plural(layout.windows, "window")),
  );
  return group;
}

function bufferTabs(layout: NvimLayout): HTMLElement {
  const list = element("div", "nvim-buffer-tabs");
  list.setAttribute("role", "tablist");
  list.setAttribute("aria-label", "Open buffers");
  for (const entry of layout.buffers) {
    const tab = element("button", "nvim-buffer-tab");
    tab.setAttribute("type", "button");
    tab.setAttribute("role", "tab");
    tab.setAttribute("aria-selected", String(entry.current));
    tab.append(element("span", "nvim-buffer-tab-name", entry.name));
    if (entry.modified) {
      const dot = element("span", "nvim-buffer-tab-modified", "●");
      dot.setAttribute("aria-label", "Modified");
      tab.append(dot);
    }
    list.append(tab);
  }
  return list;
}

function render(dom: HTMLElement, snapshot: BindingSnapshot): void {
  const { layout } = snapshot.overlays;
  const show = snapshot.connection.status === "attached" && isInformative(layout);
  dom.replaceChildren(...(show ? [tabpageGroup(layout), bufferTabs(layout)] : []));
  dom.hidden = !show;
}

export const nvimTablinePanel = (view: EditorView): Panel => {
  const dom = element("div", "cm-nvim-tabline-panel");
  render(dom, view.state.field(bindingStateField));
  return {
    dom,
    top: true,
    update(update) {
      const next = update.state.field(bindingStateField);
      if (next !== update.startState.field(bindingStateField)) render(dom, next);
    },
  };
};

export const nvimTablinePanelExtension = [
  showPanel.of(nvimTablinePanel),
  EditorView.baseTheme({
    ".cm-nvim-tabline-panel": {
      display: "flex",
      alignItems: "center",
      gap: "8px",
      minHeight: "22px",
      padding: "0 8px",
      borderBottom: "1px solid var(--color-border-subtle)",
      backgroundColor: "var(--color-bg-panel)",
      fontSize: "11px",
      fontFamily: "var(--font-mono, monospace)",
    },
    ".cm-nvim-tabline-panel[hidden]": { display: "none" },
    ".nvim-tabpage-group": { display: "flex", gap: "6px", color: "var(--color-text-muted)" },
    ".nvim-buffer-tabs": { display: "flex", gap: "4px", overflowX: "auto" },
    ".nvim-buffer-tab": {
      display: "inline-flex",
      alignItems: "center",
      gap: "4px",
      padding: "0 6px",
      border: "1px solid transparent",
      borderRadius: "var(--radius, 6px)",
      background: "transparent",
      color: "var(--color-text-muted)",
      font: "inherit",
      cursor: "default",
    },
    ".nvim-buffer-tab[aria-selected=true]": {
      borderColor: "var(--color-border-subtle)",
      color: "var(--color-text)",
    },
    ".nvim-buffer-tab-modified": { color: "var(--color-warning)" },
  }),
];
