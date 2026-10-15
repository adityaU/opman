/**
 * "When a runner reaches its limit": ask, switch on its own, or stay quiet. Held by the
 * server with the rest of the limit settings — see `settingsStore.ts`.
 */
import { useSyncExternalStore } from "react";
import type { LimitPolicy } from "../api/runnerLimits";
import { getSettings, subscribeSettings, updateSettings } from "./settingsStore";

export type { LimitPolicy };

const policyOf = () => getSettings().policy;
const setPolicy = (policy: LimitPolicy) => updateSettings({ policy });

export function useLimitPolicy(): [LimitPolicy, (policy: LimitPolicy) => void] {
  const policy = useSyncExternalStore(subscribeSettings, policyOf, policyOf);
  return [policy, setPolicy];
}
