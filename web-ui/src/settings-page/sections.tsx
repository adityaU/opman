import React from "react";
import { Keyboard, Palette, Plug, Sparkles } from "lucide-react";
import type { SettingsSection } from "./useSettingsRoute";

/**
 * The four things settings configures.
 *
 * Configuration only. Routines, session instructions and memory are content — they keep
 * their own surfaces, because the question "what does opman remember" is not the same
 * question as "how is opman set up".
 */

export interface SectionMeta {
  readonly id: SettingsSection;
  readonly label: string;
  /** One line in the rail. Says what the section decides, not what it contains. */
  readonly summary: string;
  readonly icon: React.ReactNode;
}

export const SECTIONS: readonly SectionMeta[] = [
  {
    id: "appearance",
    label: "Appearance",
    summary: "Theme, light or dark, glassy or flat",
    icon: <Palette size={15} />,
  },
  {
    id: "keybindings",
    label: "Keybindings",
    summary: "Every shortcut, and what it is bound to",
    icon: <Keyboard size={15} />,
  },
  {
    id: "mcp",
    label: "MCP Servers",
    summary: "Tools every runner can reach",
    icon: <Plug size={15} />,
  },
  {
    id: "skills",
    label: "Skills",
    summary: "Reusable instructions agents can load",
    icon: <Sparkles size={15} />,
  },
];
