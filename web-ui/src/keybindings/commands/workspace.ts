import type { CommandDef } from "../types";

/**
 * Workspace commands — panes and windows.
 *
 * The vocabulary is deliberately borrowed rather than invented: a pane is a
 * VSCode editor group and a vim window, a window is a VSCode editor layout and
 * a vim tab. Titles read the way VSCode's command palette phrases the same
 * action, so searching the palette for "split editor" finds the right thing.
 *
 * Everything the pointer can do to the workspace has an entry here. That is the
 * contract behind "fully keyboard navigable": the pane menu, the rail and the
 * opener are all views onto this list, not separate capabilities.
 */

const ORDINALS = [1, 2, 3, 4, 5, 6, 7, 8, 9] as const;

/** `mod+1..9` / `<leader>1..9` — jump straight to a pane by its badge number. */
const FOCUS_ORDINALS: readonly CommandDef[] = ORDINALS.map((n) => ({
  id: `workspace.focusPane${n}`,
  title: `Focus Pane ${n}`,
  category: "Workspace",
  label: `pane ${n}`,
}));

export const WORKSPACE_COMMANDS: readonly CommandDef[] = [
  // ── Splitting ──
  { id: "workspace.splitRight", title: "Split Pane Right", category: "Workspace", label: "split right" },
  { id: "workspace.splitDown", title: "Split Pane Down", category: "Workspace", label: "split down" },

  // ── Pane lifecycle ──
  { id: "workspace.closePane", title: "Close Pane", category: "Workspace", label: "close" },
  { id: "workspace.closeOtherPanes", title: "Close Other Panes", category: "Workspace", label: "only" },
  { id: "workspace.zoomPane", title: "Toggle Pane Zoom", category: "Workspace", label: "zoom" },
  { id: "workspace.equalize", title: "Reset Pane Sizes", category: "Workspace", label: "equalize" },

  // ── Focus ──
  ...FOCUS_ORDINALS,
  { id: "workspace.focusLeft", title: "Focus Pane Left", category: "Workspace", label: "left" },
  { id: "workspace.focusRight", title: "Focus Pane Right", category: "Workspace", label: "right" },
  { id: "workspace.focusUp", title: "Focus Pane Up", category: "Workspace", label: "up" },
  { id: "workspace.focusDown", title: "Focus Pane Down", category: "Workspace", label: "down" },
  { id: "workspace.cyclePane", title: "Focus Next Pane", category: "Workspace", label: "cycle" },

  // ── Moving ──
  { id: "workspace.movePaneLeft", title: "Move Pane Left", category: "Workspace", label: "move left" },
  { id: "workspace.movePaneRight", title: "Move Pane Right", category: "Workspace", label: "move right" },
  { id: "workspace.movePaneUp", title: "Move Pane Up", category: "Workspace", label: "move up" },
  { id: "workspace.movePaneDown", title: "Move Pane Down", category: "Workspace", label: "move down" },
  {
    id: "workspace.movePaneToNewWindow",
    title: "Move Pane to New Window",
    category: "Workspace",
    label: "to new window",
  },

  // ── Windows ──
  { id: "workspace.newWindow", title: "New Window", category: "Workspace", label: "new window" },
  { id: "workspace.closeWindow", title: "Close Window", category: "Workspace", label: "close window" },
  { id: "workspace.renameWindow", title: "Rename Window", category: "Workspace", label: "rename" },
  { id: "workspace.nextWindow", title: "Next Window", category: "Workspace", label: "next window" },
  { id: "workspace.previousWindow", title: "Previous Window", category: "Workspace", label: "prev window" },
  { id: "workspace.windowSwitcher", title: "Go to Window…", category: "Workspace", label: "go to window" },

  // ── Opening things ──
  { id: "workspace.openWidget", title: "Open Widget in Pane…", category: "Workspace", label: "open" },
  { id: "workspace.paneMenu", title: "Show Pane Menu", category: "Workspace", label: "menu" },

  // ── Chrome ──
  { id: "workspace.toggleRail", title: "Toggle Window Rail", category: "Workspace", label: "rail" },
  { id: "workspace.toggleZen", title: "Toggle Zen", category: "Workspace", label: "zen" },
  {
    id: "workspace.togglePaneHeaders",
    title: "Toggle Pane Headers",
    category: "Workspace",
    label: "headers",
  },
];

/**
 * Keys live only while an overlay is up. They are commands rather than a local
 * keydown listener so the same `when` machinery, the same config file and the
 * same keybindings view cover them — an overlay is not a place where the
 * keymap stops applying.
 */
export const WORKSPACE_OVERLAY_COMMANDS: readonly CommandDef[] = [
  ...ORDINALS.map((n) => ({
    id: `workspace.targetPane${n}`,
    title: `Send to Pane ${n}`,
    category: "Workspace",
    when: "workspaceTargeting",
    label: `pane ${n}`,
  })),
  {
    id: "workspace.targetAccept",
    title: "Send to Focused Pane",
    category: "Workspace",
    when: "workspaceTargeting",
    label: "accept",
  },
  {
    id: "workspace.targetCancel",
    title: "Cancel Targeting",
    category: "Workspace",
    when: "workspaceTargeting",
    label: "cancel",
  },
  {
    id: "workspace.targetSplitDown",
    title: "Send to a New Pane Below",
    category: "Workspace",
    when: "workspaceTargeting",
    label: "split down",
  },
  {
    id: "workspace.targetSplitRight",
    title: "Send to a New Pane Right",
    category: "Workspace",
    when: "workspaceTargeting",
    label: "split right",
  },
  {
    id: "workspace.targetNewWindow",
    title: "Send to a New Window",
    category: "Workspace",
    when: "workspaceTargeting",
    label: "new window",
  },
];
