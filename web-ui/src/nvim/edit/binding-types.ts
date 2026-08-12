import type { BindingSnapshot, IdleReason } from "./decorations";

export interface BindingOptions {
  readonly enabled: boolean;
  readonly path: string | null;
  readonly sessionId: string | null | undefined;
  readonly idleReason: IdleReason;
  readonly onBufferDetached?: () => void;
  /** Run an editor action a Neovim mapping asked for, by name. */
  readonly onAction?: (name: string) => void;
}

export function isInsertMode(mode: BindingSnapshot["modeShort"]): boolean {
  return mode === "insert";
}

export function editId(sequence: number): string {
  return `opman-edit-${sequence.toString(36)}-${Date.now().toString(36)}`;
}
