import type { BindingSpec, Layer } from "../types";
import {
  BASE_ASSISTANT,
  BASE_ENGINE,
  BASE_LAYOUT,
  BASE_PALETTE,
  BASE_SYSTEM,
} from "./base-core";
import { BASE_CHAT, BASE_SESSION } from "./base-session";
import { BASE_EDITOR, BASE_EXPLORER, BASE_LSP, BASE_RICH_FILE } from "./base-editor";
import { BASE_BOARD, BASE_GIT, BASE_TERMINAL } from "./base-panels";
import { PLATFORM_LAYER } from "./platform";
import { WEB_LAYER } from "./web";
import { VIM_LAYER } from "./vim";

/** The canonical keymap, before any host bends it. */
export const BASE_BINDINGS: readonly BindingSpec[] = [
  ...BASE_PALETTE,
  ...BASE_LAYOUT,
  ...BASE_SESSION,
  ...BASE_CHAT,
  ...BASE_ENGINE,
  ...BASE_EDITOR,
  ...BASE_LSP,
  ...BASE_RICH_FILE,
  ...BASE_EXPLORER,
  ...BASE_TERMINAL,
  ...BASE_GIT,
  ...BASE_BOARD,
  ...BASE_ASSISTANT,
  ...BASE_SYSTEM,
];

/**
 * The built-in layers, in application order. Config and user layers are
 * appended by the caller, which is what keeps `resolve` free of any I/O.
 *
 * The vim bindings sit inside the base layer rather than after the overrides:
 * they are canonical too, and a later layer has to be able to move one when a
 * host steals its chord. Superseding is cross-layer only, so sharing a layer
 * with the normal-mode bindings costs nothing — the two never match the same
 * (command, when, mode).
 */
export function builtInLayers(): Layer[] {
  return [
    { source: "base", bindings: [...BASE_BINDINGS, ...VIM_LAYER] },
    { source: "platform", bindings: PLATFORM_LAYER },
    { source: "target", bindings: WEB_LAYER },
  ];
}

export { PLATFORM_LAYER, WEB_LAYER, VIM_LAYER };
