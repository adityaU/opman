import type { CommandDef } from "../types";

/**
 * Directional focus across the whole shell, and traversal of the sidebar list.
 *
 * `nav.focus*` is deliberately not `workspace.focus*`. The workspace commands
 * move between panes and stop at the edge of the tree, which is the right
 * behaviour for `Ctrl+K ←` — a pane command, in the pane namespace. These move
 * between whatever is adjacent, tree or not, and fall back to the pane
 * neighbour logic while they happen to be inside the tree.
 */
export const NAV_COMMANDS: readonly CommandDef[] = [
  { id: "nav.focusLeft", title: "Focus Left", category: "Navigation", label: "left" },
  { id: "nav.focusRight", title: "Focus Right", category: "Navigation", label: "right" },
  { id: "nav.focusUp", title: "Focus Up", category: "Navigation", label: "up" },
  { id: "nav.focusDown", title: "Focus Down", category: "Navigation", label: "down" },
];

/**
 * The sidebar's list keys. Scoped to `focus==sidebar` because they are bare —
 * the same contract the explorer and the git panel already work under.
 */
export const SIDEBAR_COMMANDS: readonly CommandDef[] = [
  {
    id: "sidebar.moveDown",
    title: "Next Sidebar Row",
    category: "Sidebar",
    when: "focus==sidebar",
    label: "down",
  },
  {
    id: "sidebar.moveUp",
    title: "Previous Sidebar Row",
    category: "Sidebar",
    when: "focus==sidebar",
    label: "up",
  },
  {
    id: "sidebar.expand",
    title: "Expand Sidebar Group",
    category: "Sidebar",
    when: "focus==sidebar",
    label: "expand",
  },
  {
    id: "sidebar.collapse",
    title: "Collapse Sidebar Group",
    category: "Sidebar",
    when: "focus==sidebar",
    label: "collapse",
  },
  {
    id: "sidebar.open",
    title: "Open Selected Session",
    category: "Sidebar",
    when: "focus==sidebar",
    label: "open",
  },
];
