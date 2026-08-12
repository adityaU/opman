import { useEffect, useMemo } from "react";
import type { Extension } from "@codemirror/state";
import { NvimEditBinding } from "./binding";
import type { IdleReason } from "./decorations";

export interface NvimEditBindingOptions {
  readonly enabled: boolean;
  readonly path: string | null;
  readonly sessionId: string | null | undefined;
  readonly idleReason: IdleReason;
  readonly onBufferDetached?: () => void;
  readonly onAction?: (name: string) => void;
}

/** Mount the V1 text binding while leaving CodeMirror's renderer in place. */
export function useNvimEditBinding(options: NvimEditBindingOptions): Extension[] {
  const binding = useMemo(() => new NvimEditBinding(), []);
  useEffect(() => {
    binding.setOptions(options);
    return () => binding.setOptions({
      enabled: false, path: null, sessionId: null, idleReason: "disabled",
      onBufferDetached: undefined, onAction: undefined,
    });
  }, [binding, options.enabled, options.path, options.sessionId, options.idleReason,
      options.onBufferDetached, options.onAction]);
  useEffect(() => () => binding.dispose(), [binding]);
  const extensions = useMemo(() => [binding.extension], [binding]);
  return options.path !== null ? extensions : [];
}
