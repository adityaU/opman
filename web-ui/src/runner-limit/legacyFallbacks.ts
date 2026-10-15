/**
 * The format an earlier build stored the fallback list in, per browser. Only read once,
 * to move it to the server (see `settingsStore.ts`). Never throws on odd shapes.
 */
import type { FallbackModel, FallbackPair, RunnerLimitSettings } from "../api/runnerLimits";
import { NEAR_PERCENT } from "./limitState";

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null;

function readModel(value: unknown): FallbackModel | null {
  if (!isObject(value)) return null;
  const { providerID, modelID } = value;
  return typeof providerID === "string" && typeof modelID === "string" && modelID
    ? { providerID, modelID }
    : null;
}

function readPair(value: unknown): FallbackPair | null {
  if (!isObject(value) || typeof value.runner !== "string" || !value.runner) return null;
  return { runner: value.runner, model: readModel(value.model) };
}

export type LegacyFallbacks = Pick<RunnerLimitSettings, "nearPercent" | "pairs">;

export const LEGACY_DEFAULTS: LegacyFallbacks = { nearPercent: NEAR_PERCENT, pairs: [] };

export function parseFallbacks(raw: string | null): LegacyFallbacks {
  if (!raw) return LEGACY_DEFAULTS;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!isObject(parsed)) return LEGACY_DEFAULTS;
    const near = Number(parsed.nearPercent);
    const pairs = Array.isArray(parsed.pairs)
      ? parsed.pairs.map(readPair).filter((pair): pair is FallbackPair => pair !== null)
      : [];
    return {
      nearPercent: Number.isInteger(near) && near > 0 && near < 100 ? near : NEAR_PERCENT,
      pairs,
    };
  } catch {
    return LEGACY_DEFAULTS;
  }
}
