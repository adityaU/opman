import type { BindingSpec } from "../types";

/**
 * Platform overrides.
 *
 * `mod+` already covers the Command/Control difference, so this layer is only
 * for the cases where macOS and Windows genuinely disagree about which chord an
 * action wants — not for translating a modifier.
 */

const MAC: readonly BindingSpec[] = [
  // Cmd+H hides the application, so VSCode puts Replace on Option+Cmd+F.
  { key: "alt+meta+f", command: "editor.replace", when: "focus==editor" },
  // Cmd+Up / Cmd+Down are the macOS document-extent keys.
  { key: "meta+up", command: "chat.scrollTop", when: "focus==chat" },
  { key: "meta+down", command: "chat.scrollBottom", when: "focus==chat" },
].map((spec) => ({ ...spec, platform: "mac" as const }));

export const PLATFORM_LAYER: readonly BindingSpec[] = [...MAC];
