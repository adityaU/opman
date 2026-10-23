/**
 * Where to send a conversation whose runner is running out.
 *
 * The user's own list from settings comes first, in their order, but only the pairs whose
 * runner would actually get further (see `rankAlternatives`: not the same account, not
 * itself out). After those come the other runners the usage report says have room, each
 * on its default model — the suggestions. The first option is the one to offer, or to
 * switch to on its own when the policy says so and it came from the user's list.
 */
import type { UsageReport } from "../usage/usageClient";
import { rankAlternatives, type Alternative } from "./alternatives";
import type { FallbackModel, FallbackPair } from "./fallbacks";
import type { LimitLevel } from "./limitState";
import type { ModelSet } from "../api/runnerLimits";

export type TargetSource = "preselected" | "suggested";

export interface LimitTarget {
  readonly runner: string;
  readonly model: FallbackModel | null;
  readonly permission?: string | null;
  readonly source: TargetSource;
  /** Percent left on the target's tightest window; null when it reports no usage. */
  readonly leftPercent: number | null;
}

export interface TargetInput {
  readonly report: UsageReport | null;
  readonly current: string;
  readonly level: LimitLevel;
  readonly pairs: readonly FallbackPair[];
  readonly available: readonly string[];
  readonly now: number;
  readonly nearPercent: number;
  readonly currentModel?: FallbackModel | null;
  readonly modelSets?: readonly ModelSet[];
}

/** Every viable option, best first. Empty when there is nowhere better to go. */
export function limitOptions(input: TargetInput): LimitTarget[] {
  if (input.level === "ok") return [];
  const ranked = rankAlternatives(
    input.report, input.current, input.available, input.now, input.level, input.nearPercent,
  );
  const viable = new Map<string, Alternative>(ranked.map((alt) => [alt.runner, alt]));
  const options: LimitTarget[] = [];
  const matchingSet = input.currentModel ? input.modelSets?.find(set => set.members.some(member =>
    member.runner === input.current && member.model?.providerID === input.currentModel?.providerID
      && member.model?.modelID === input.currentModel?.modelID)) : undefined;
  for (const pair of matchingSet?.members ?? input.pairs) {
    const alt = viable.get(pair.runner);
    if (!alt) continue;
    options.push({ ...pair, source: "preselected", leftPercent: alt.leftPercent });
  }
  if (matchingSet) return options;
  const listed = new Set(options.map((option) => option.runner));
  for (const alt of ranked) {
    if (listed.has(alt.runner)) continue;
    options.push({ runner: alt.runner, model: null, source: "suggested", leftPercent: alt.leftPercent });
  }
  return options;
}

export function optionKey(option: Pick<LimitTarget, "runner" | "model">): string {
  return option.model ? `${option.runner}|${option.model.providerID}|${option.model.modelID}` : option.runner;
}
