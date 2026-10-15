/**
 * Move the composer to another runner, model and permission, then optionally retry.
 *
 * The composer's setters only take effect on the next render — a runner change clears
 * the model, and a send reads the engine it was rendered with — so these steps run as
 * a small state machine driven by what the composer reports back, not back to back in one
 * click handler. Each step waits for the previous one to be visible in the props.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import type { FallbackModel } from "./fallbacks";

export interface SwitchTarget {
  readonly runner: string;
  readonly model: FallbackModel | null;
  readonly permission?: string | null;
}

type Pending =
  | { readonly stage: "runner"; readonly target: SwitchTarget; readonly prompt: string | null }
  | {
      readonly stage: "model";
      readonly target: SwitchTarget;
      readonly prompt: string | null;
      readonly tries: number;
      /** The model on screen when the last request went out; a change from it is news. */
      readonly seen: FallbackModel | null;
    }
  | { readonly stage: "permission"; readonly target: SwitchTarget; readonly prompt: string | null }
  | { readonly stage: "send"; readonly target: SwitchTarget; readonly prompt: string };

/** The composer replaced our model with another this many times: leave it be. */
const MODEL_TRIES = 3;
/** How long to wait for the model to show before carrying on without it. */
export const MODEL_WAIT_MS = 2_000;

export interface LimitSwitchHost {
  readonly currentRunner: string;
  readonly selectedModel: FallbackModel | null;
  readonly selectedPermission?: string;
  readonly onRunnerChange?: (runner: string) => void;
  readonly onModelSelected?: (modelId: string, providerId: string) => void;
  readonly onPermissionChange?: (permission: string) => void;
  readonly onSend: (text: string) => Promise<boolean>;
}

const sameModel = (a: FallbackModel | null, b: FallbackModel | null) =>
  a?.modelID === b?.modelID && a?.providerID === b?.providerID;

function afterPermission(target: SwitchTarget, prompt: string | null): Pending | null {
  return prompt ? { stage: "send", target, prompt } : null;
}

export function useLimitSwitch(host: LimitSwitchHost) {
  const [pending, setPending] = useState<Pending | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Callbacks are read at the moment a step runs, so a step uses the setters and the send
  // of the render that made it ready.
  const latest = useRef(host);
  latest.current = host;

  const switchTo = useCallback((target: SwitchTarget, prompt: string | null = null) => {
    const { currentRunner, onRunnerChange } = latest.current;
    if (target.runner !== currentRunner) onRunnerChange?.(target.runner);
    setError(null);
    setPending({ stage: "runner", target, prompt });
  }, []);

  const { currentRunner, selectedModel, selectedPermission } = host;
  useEffect(() => {
    if (!pending) return;
    const { onModelSelected, onPermissionChange, onSend } = latest.current;
    if (currentRunner !== pending.target.runner) return;
    if (pending.stage === "send") {
      if (pending.target.permission && selectedPermission !== pending.target.permission) {
        setPending({ stage: "permission", target: pending.target, prompt: pending.prompt });
        return;
      }
      setPending(null);
      void onSend(pending.prompt);
      return;
    }
    if (pending.stage === "permission") {
      const permission = pending.target.permission;
      if (!permission || selectedPermission === permission) {
        setPending(afterPermission(pending.target, pending.prompt));
        return;
      }
      onPermissionChange?.(permission);
      return;
    }
    const model = pending.target.model;
    if (!model || sameModel(selectedModel, model)) {
      setPending({ stage: "permission", target: pending.target, prompt: pending.prompt });
      return;
    }
    // Still waiting on our own request: nothing on screen has moved since it went out.
    if (pending.stage === "model" && sameModel(selectedModel, pending.seen)) return;
    const tries = pending.stage === "model" ? pending.tries : 0;
    if (tries >= MODEL_TRIES) {
      setPending({ stage: "permission", target: pending.target, prompt: pending.prompt });
      return;
    }
    onModelSelected?.(model.modelID, model.providerID);
    setPending({ stage: "model", target: pending.target, prompt: pending.prompt, tries: tries + 1, seen: selectedModel });
  }, [pending, currentRunner, selectedModel, selectedPermission]);

  // A host that never reports the model back must not hold the prompt forever.
  const waitingOnModel = pending?.stage === "model";
  useEffect(() => {
    if (!waitingOnModel) return;
    const timer = window.setTimeout(() => {
      setPending((current) => (current?.stage === "model"
        ? { stage: "permission", target: current.target, prompt: current.prompt } : current));
    }, MODEL_WAIT_MS);
    return () => window.clearTimeout(timer);
  }, [waitingOnModel]);

  // Never retry a turn with a different permission than the user chose.
  const waitingOnPermission = pending?.stage === "permission";
  useEffect(() => {
    if (!waitingOnPermission) return;
    const timer = window.setTimeout(() => {
      setPending(null);
      setError("Permission could not be applied. Check the runner settings before retrying.");
    }, MODEL_WAIT_MS);
    return () => window.clearTimeout(timer);
  }, [waitingOnPermission]);

  const clearError = useCallback(() => setError(null), []);
  return { switchTo, switching: pending !== null, error, clearError };
}
