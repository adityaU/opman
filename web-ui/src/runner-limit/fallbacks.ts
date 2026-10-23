/**
 * Where a conversation goes when its runner runs low: an ordered list of runner + model
 * pairs and the percentage at which "low" starts. Held by the server — see
 * `settingsStore.ts` — and read here by the composer and the settings page.
 */
import { useSyncExternalStore } from "react";
import type { FallbackModel, FallbackPair, ModelSet } from "../api/runnerLimits";
import { getSettings, subscribeSettings, updateSettings } from "./settingsStore";

export type { FallbackModel, FallbackPair };

/** The thresholds settings offers. Above a quarter a warning stops meaning anything. */
export const NEAR_PERCENT_CHOICES = [2, 5, 10, 15, 20, 25] as const;

export interface FallbackSettings {
  readonly nearPercent: number;
  readonly pairs: readonly FallbackPair[];
  readonly modelSets?: readonly ModelSet[];
}

/** Same runner and same model: listing a pair twice would only try it twice. */
export function samePair(a: FallbackPair, b: FallbackPair): boolean {
  return a.runner === b.runner && a.model?.providerID === b.model?.providerID
    && a.model?.modelID === b.model?.modelID;
}

export function persistFallbacks(next: FallbackSettings): void {
  updateSettings({ nearPercent: next.nearPercent, pairs: next.pairs });
}

export function useFallbacks(): FallbackSettings {
  return useSyncExternalStore(subscribeSettings, getSettings, getSettings);
}
