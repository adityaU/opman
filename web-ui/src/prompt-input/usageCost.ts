import type { SessionStats } from "../api";

/** Relative per-token price weights, consistent across the Claude model family:
 *  output ≈ 5x input, cache write ≈ 1.25x input, cache read ≈ 0.1x input. */
const WEIGHT = {
  input: 1,
  output: 5,
  reasoning: 5,
  cache_write: 1.25,
  cache_read: 0.1,
} as const;

export interface UsageRow {
  key: keyof typeof WEIGHT;
  label: string;
  color: string;
  tokens: number;
  pct: number;
  cost: number;
}

export interface UsageBreakdown {
  rows: UsageRow[];
  totalTokens: number;
  totalCost: number;
}

/**
 * Splits a session's total cost across token categories using the standard
 * Claude relative pricing ratios. The split is an estimate (exact per-model
 * per-token prices aren't available client-side) but always sums exactly to
 * the real reported `stats.cost`.
 */
export function computeUsageBreakdown(stats: SessionStats): UsageBreakdown {
  const tokensByKey: Record<keyof typeof WEIGHT, number> = {
    input: stats.input_tokens,
    output: stats.output_tokens,
    reasoning: stats.reasoning_tokens,
    cache_write: stats.cache_write,
    cache_read: stats.cache_read,
  };
  const totalTokens = Object.values(tokensByKey).reduce((a, b) => a + b, 0);
  const totalWeighted = (Object.keys(tokensByKey) as (keyof typeof WEIGHT)[])
    .reduce((sum, k) => sum + tokensByKey[k] * WEIGHT[k], 0);

  const meta: Record<keyof typeof WEIGHT, { label: string; color: string }> = {
    input: { label: "Input", color: "blue" },
    output: { label: "Output", color: "purple" },
    reasoning: { label: "Reasoning", color: "gray" },
    cache_write: { label: "Cache write", color: "orange" },
    cache_read: { label: "Cache read", color: "green" },
  };

  const rows: UsageRow[] = (Object.keys(tokensByKey) as (keyof typeof WEIGHT)[])
    .filter((k) => tokensByKey[k] > 0)
    .map((k) => {
      const tokens = tokensByKey[k];
      const weighted = tokens * WEIGHT[k];
      return {
        key: k,
        label: meta[k].label,
        color: meta[k].color,
        tokens,
        pct: totalTokens > 0 ? (tokens / totalTokens) * 100 : 0,
        cost: totalWeighted > 0 ? stats.cost * (weighted / totalWeighted) : 0,
      };
    });

  return { rows, totalTokens, totalCost: stats.cost };
}

/** Format a dollar amount, matching the StatusBar's 4-decimal convention. */
export function formatCost(n: number): string {
  return `$${n.toFixed(4)}`;
}
