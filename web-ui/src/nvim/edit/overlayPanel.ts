import { EditorView, showPanel, type Panel } from "@codemirror/view";
import { bindingStateField, type BindingSnapshot } from "./decorations";
import { messageTone, showmodeLabel, type CmdlineOverlay, type NvimMessage } from "./overlays";

function element(tag: string, className: string, text?: string): HTMLElement {
  const node = document.createElement(tag);
  node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function cmdline(state: CmdlineOverlay): HTMLElement {
  const section = element("section", "nvim-cmdline-overlay");
  section.setAttribute("aria-label", "Neovim command line");
  section.setAttribute("aria-live", "polite");
  const line = element("div", "nvim-cmdline-line");
  line.dataset.position = String(state.position);
  line.append(element("span", "nvim-cmdline-firstc", state.firstChar));
  const pivot = Math.max(0, Math.min(state.position, state.content.length));
  const content = element("span", "nvim-cmdline-content");
  content.append(
    element("span", "nvim-cmdline-before", state.content.slice(0, pivot)),
    element("span", "nvim-cmdline-cursor"),
    element("span", "nvim-cmdline-after", state.content.slice(pivot)),
  );
  content.querySelector(".nvim-cmdline-cursor")?.setAttribute("aria-hidden", "true");
  line.append(content);
  section.append(line);
  return section;
}

function messages(items: readonly NvimMessage[]): HTMLElement {
  const section = element("section", "nvim-messages-overlay");
  section.setAttribute("aria-label", "Neovim messages");
  section.setAttribute("aria-live", "polite");
  const list = element("div", "nvim-message-list");
  for (const item of items) {
    const tone = messageTone(item.kind);
    const row = element("div", `nvim-message nvim-message-${tone}`);
    if (tone !== "info") row.setAttribute("role", tone === "error" ? "alert" : "status");
    row.append(element("span", "nvim-message-content", item.text));
    list.append(row);
  }
  section.append(list);
  return section;
}

function statusline(snapshot: BindingSnapshot): HTMLElement | null {
  const label = showmodeLabel(snapshot.modeShort);
  if (label === null) return null;
  const footer = element("footer", "nvim-statusline-overlay");
  footer.setAttribute("aria-label", "Neovim status");
  footer.append(element("span", "nvim-statusline-showmode", label));
  return footer;
}

function render(dom: HTMLElement, snapshot: BindingSnapshot): void {
  const { cmdline: line, messages: items } = snapshot.overlays;
  const children: HTMLElement[] = [];
  if (items.length > 0) children.push(messages(items));
  if (line.visible) children.push(cmdline(line));
  const status = statusline(snapshot);
  if (status) children.push(status);
  dom.replaceChildren(...children);
  dom.hidden = children.length === 0;
}

/** The command line, message list and showmode line, below the document. */
export const nvimOverlayPanel = (view: EditorView): Panel => {
  const dom = element("div", "cm-nvim-overlay-panel");
  render(dom, view.state.field(bindingStateField));
  return {
    dom,
    update(update) {
      const next = update.state.field(bindingStateField);
      if (next !== update.startState.field(bindingStateField)) render(dom, next);
    },
  };
};

export const nvimOverlayPanelExtension = [
  showPanel.of(nvimOverlayPanel),
  EditorView.baseTheme({
    ".cm-nvim-overlay-panel": {
      display: "flex",
      flexDirection: "column",
      gap: "2px",
      padding: "2px 8px",
      borderTop: "1px solid var(--color-border-subtle)",
      backgroundColor: "var(--color-bg-panel)",
      fontSize: "11px",
      fontFamily: "var(--font-mono, monospace)",
    },
    ".cm-nvim-overlay-panel[hidden]": { display: "none" },
    ".nvim-cmdline-overlay": { display: "flex", color: "var(--color-text)" },
    ".nvim-cmdline-line": { display: "flex", alignItems: "center", whiteSpace: "pre" },
    ".nvim-cmdline-firstc": { color: "var(--color-primary)", fontWeight: "600" },
    ".nvim-cmdline-content": { display: "inline-flex", whiteSpace: "pre" },
    ".nvim-cmdline-cursor": {
      display: "inline-block",
      width: "1px",
      alignSelf: "stretch",
      backgroundColor: "var(--color-primary)",
    },
    ".nvim-messages-overlay": { display: "flex", maxHeight: "84px", overflowY: "auto" },
    ".nvim-message-list": { display: "flex", flexDirection: "column", gap: "1px", width: "100%" },
    ".nvim-message": { whiteSpace: "pre-wrap", color: "var(--color-text-muted)" },
    ".nvim-message-error": { color: "var(--color-danger, var(--color-warning))" },
    ".nvim-message-warning": { color: "var(--color-warning)" },
    ".nvim-statusline-showmode": { color: "var(--color-success)", fontWeight: "600" },
  }),
];
