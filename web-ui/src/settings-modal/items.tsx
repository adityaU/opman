import React from "react";
import { Palette, Monitor, Keyboard, Bell, Brain, Bot, Clock3 } from "lucide-react";

export interface SettingItem {
  id: string;
  label: string;
  description: string;
  type: "toggle" | "action";
  value?: boolean;
  icon: React.ReactNode;
  handler: () => void;
}

interface BuildItemsArgs {
  onClose: () => void;
  onOpenThemeSelector: () => void;
  onOpenCheatsheet: () => void;
  onOpenNotificationPrefs?: () => void;
  onOpenMemory?: () => void;
  onOpenAutonomy?: () => void;
  onOpenRoutines?: () => void;
  sidebarOpen: boolean;
  terminalOpen: boolean;
  neovimOpen: boolean;
  gitOpen: boolean;
  onToggleSidebar: () => void;
  onToggleTerminal: () => void;
  onToggleNeovim: () => void;
  onToggleGit: () => void;
}

export function buildSettingsItems(args: BuildItemsArgs): SettingItem[] {
  const {
    onClose,
    onOpenThemeSelector,
    onOpenCheatsheet,
    onOpenNotificationPrefs,
    onOpenMemory,
    onOpenAutonomy,
    onOpenRoutines,
    sidebarOpen,
    terminalOpen,
    neovimOpen,
    gitOpen,
    onToggleSidebar,
    onToggleTerminal,
    onToggleNeovim,
    onToggleGit,
  } = args;

  const action = (fn?: () => void) => () => { onClose(); fn?.(); };

  return [
    { id: "theme", label: "Theme", description: "Choose a color theme", type: "action", icon: <Palette size={14} />, handler: action(onOpenThemeSelector) },
    { id: "keybindings", label: "Keybindings", description: "View keyboard shortcuts", type: "action", icon: <Keyboard size={14} />, handler: action(onOpenCheatsheet) },
    { id: "notifications", label: "Notifications", description: "Configure session alerts", type: "action", icon: <Bell size={14} />, handler: action(onOpenNotificationPrefs) },
    { id: "memory", label: "Session Instructions", description: "Standing guidance sent when a session opens", type: "action", icon: <Brain size={14} />, handler: action(onOpenMemory) },
    { id: "autonomy", label: "Autonomy", description: "Choose how proactive opman may be", type: "action", icon: <Bot size={14} />, handler: action(onOpenAutonomy) },
    { id: "routines", label: "Routines", description: "Manage scheduled and triggered routines", type: "action", icon: <Clock3 size={14} />, handler: action(onOpenRoutines) },
    { id: "sidebar", label: "Sidebar", description: "Show/hide the sidebar panel", type: "toggle", value: sidebarOpen, icon: <Monitor size={14} />, handler: onToggleSidebar },
    { id: "terminal", label: "Terminal", description: "Show/hide the terminal panel", type: "toggle", value: terminalOpen, icon: <Monitor size={14} />, handler: onToggleTerminal },
    { id: "neovim", label: "Editor", description: "Show/hide the code editor panel", type: "toggle", value: neovimOpen, icon: <Monitor size={14} />, handler: onToggleNeovim },
    { id: "git", label: "Git Panel", description: "Show/hide the git panel", type: "toggle", value: gitOpen, icon: <Monitor size={14} />, handler: onToggleGit },
  ];
}
