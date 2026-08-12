import { EditorView, showPanel, type Panel } from "@codemirror/view";
import { bindingStateField, type BindingSnapshot, type IdleReason } from "./decorations";

function modeLabel(snapshot: BindingSnapshot): string {
  return snapshot.modeShort.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function idleLabel(reason: IdleReason): string {
  switch (reason) {
    case "engine-codemirror": return "CodeMirror";
    case "mobile-surface": return "Mobile editor";
    case "no-file": return "No file open";
    case "no-session": return "No Neovim session";
    case "not-code-surface": return "Preview surface";
    case "disabled": return "Neovim idle";
  }
}

function connectionLabel(snapshot: BindingSnapshot, unavailable: boolean): string {
  switch (snapshot.connection.status) {
    case "attached": return "Neovim";
    case "connecting": return unavailable ? "Neovim unavailable" : "Connecting to Neovim…";
    case "idle": return idleLabel(snapshot.connection.reason);
    case "failed": return "Neovim unavailable";
  }
}

function failureMessage(reason: string): string {
  return reason === "Neovim connection is closed" ? "Neovim exited" : reason;
}

function render(panel: HTMLElement, snapshot: BindingSnapshot): void {
  const unavailable = snapshot.connection.status === "failed"
    || (snapshot.connection.status === "connecting" && panel.dataset.nvimUnavailable === "true");
  if (snapshot.connection.status === "attached") {
    delete panel.dataset.nvimUnavailable;
    delete panel.dataset.nvimFailure;
  } else if (snapshot.connection.status === "failed") {
    panel.dataset.nvimUnavailable = "true";
    panel.dataset.nvimFailure = failureMessage(snapshot.connection.reason);
  }

  panel.replaceChildren();
  const mode = document.createElement("span");
  mode.className = "cm-nvim-mode-label";
  mode.textContent = modeLabel(snapshot);
  mode.dataset.mode = snapshot.modeShort;
  panel.append(mode);

  const connection = document.createElement("span");
  connection.className = "cm-nvim-connection-label";
  connection.textContent = connectionLabel(snapshot, unavailable);
  panel.append(connection);

  const statusMessage = unavailable
    ? snapshot.connection.status === "failed" ? failureMessage(snapshot.connection.reason) : panel.dataset.nvimFailure
    : snapshot.message;
  if (statusMessage) {
    const message = document.createElement("span");
    message.className = "cm-nvim-message-label";
    message.textContent = statusMessage;
    panel.append(message);
  }

  const hint = document.createElement("span");
  hint.className = "cm-nvim-focus-hint";
  hint.textContent = "Ctrl+Shift+Esc releases editor focus";
  panel.append(hint);
}

export const nvimStatusPanel = (view: EditorView): Panel => {
  const dom = document.createElement("div");
  dom.className = "cm-nvim-status-panel";
  dom.setAttribute("role", "status");
  dom.setAttribute("aria-live", "polite");
  render(dom, view.state.field(bindingStateField));
  return {
    dom,
    update(update) {
      if (update.state.field(bindingStateField) !== update.startState.field(bindingStateField)) {
        render(dom, update.state.field(bindingStateField));
      }
    },
  };
};

export const nvimStatusPanelExtension = [
  showPanel.of(nvimStatusPanel),
  EditorView.baseTheme({
    ".cm-nvim-status-panel": {
      display: "flex",
      alignItems: "center",
      gap: "8px",
      minHeight: "22px",
      padding: "0 8px",
      borderTop: "1px solid var(--color-border-subtle)",
      backgroundColor: "var(--color-bg-panel)",
      color: "var(--color-text-muted)",
      fontSize: "11px",
      fontFamily: "var(--font-mono, monospace)",
    },
    ".cm-nvim-mode-label": { color: "var(--color-primary)", fontWeight: "600" },
    ".cm-nvim-mode-label[data-mode^=insert]": { color: "var(--color-success)" },
    ".cm-nvim-mode-label[data-mode^=visual]": { color: "var(--color-accent)" },
    ".cm-nvim-mode-label[data-mode^=replace]": { color: "var(--color-warning)" },
    ".cm-nvim-message-label": { color: "var(--color-warning)" },
    ".cm-nvim-focus-hint": { marginLeft: "auto", color: "var(--color-text-muted)" },
    ".cm-nvim-cursor-block": {
      backgroundColor: "var(--color-primary)",
      color: "var(--color-bg)",
    },
    ".cm-nvim-cursor-bar": {
      borderLeft: "2px solid var(--color-primary)",
      marginLeft: "-1px",
    },
    ".cm-nvim-cursor-underline": {
      textDecoration: "underline 2px var(--color-primary)",
      textUnderlineOffset: "2px",
    },
    ".cm-nvim-visual-selection": {
      backgroundColor: "color-mix(in srgb, var(--color-primary) 24%, transparent)",
    },
  }),
];
