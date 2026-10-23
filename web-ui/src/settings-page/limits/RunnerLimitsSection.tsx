import React, { useCallback } from "react";
import { ArrowDown, ArrowUp, X } from "lucide-react";
import { runnerLabel } from "../../engine-picker/paletteRows";
import { shortModelName } from "../../prompt-input/helpers";
import {
  NEAR_PERCENT_CHOICES,
  persistFallbacks,
  samePair,
  useFallbacks,
  type FallbackPair,
} from "../../runner-limit/fallbacks";
import { useLimitPolicy, type LimitPolicy } from "../../runner-limit/limitPolicy";
import { FallbackAdder } from "./FallbackAdder";
import { FallbackPermission } from "./FallbackPermission";
import { ModelSetsSection } from "./ModelSetsSection";

const POLICIES: readonly [LimitPolicy, string][] = [
  ["ask", "Ask me"],
  ["auto", "Switch on its own"],
  ["off", "Do nothing"],
];

const POLICY_NOTES: Record<LimitPolicy, string> = {
  ask: "The composer shows a note with the runner to move to. Nothing changes until you pick it.",
  auto: "opman moves the chat to the first runner with room in the matching model set or fallback list — also with no browser open. Without either, it only suggests one.",
  off: "No warnings. A turn that fails on a limit still shows its error.",
};

export interface RunnerLimitsSectionProps {
  /** Runner slots on offer — the only runners a fallback can name. */
  readonly runners: readonly string[];
}

/**
 * What happens when the runner a conversation uses is about to run out of its plan.
 *
 * The list is an order, not a set: the first pair whose runner has room wins. A runner
 * billed to the same account as the one running out is skipped when it comes up, so
 * listing "Claude Code" after "Claude" is harmless but useless.
 */
export function RunnerLimitsSection({ runners }: RunnerLimitsSectionProps) {
  const [policy, setPolicy] = useLimitPolicy();
  const settings = useFallbacks();
  const { pairs, nearPercent } = settings;

  const setPairs = useCallback(
    (next: readonly FallbackPair[]) => persistFallbacks({ ...settings, pairs: next }),
    [settings],
  );
  const add = (pair: FallbackPair) => {
    const index = pairs.findIndex((existing) => samePair(existing, pair));
    if (index >= 0) {
      setPairs(pairs.map((existing, at) => at === index ? pair : existing));
      return;
    }
    setPairs([...pairs, pair]);
  };
  const move = (index: number, by: -1 | 1) => {
    const target = index + by;
    if (target < 0 || target >= pairs.length) return;
    const next = [...pairs];
    [next[index], next[target]] = [next[target], next[index]];
    setPairs(next);
  };
  const remove = (index: number) => setPairs(pairs.filter((_, at) => at !== index));

  return (
    <div className="stg-stack">
      <section className="stg-card">
        <h3 className="stg-card-title">When a runner runs low</h3>
        <div className="stg-segmented" role="group" aria-label="When a runner runs low">
          {POLICIES.map(([value, label]) => (
            <button
              key={value}
              type="button"
              className={value === policy ? "stg-seg is-active" : "stg-seg"}
              aria-pressed={value === policy}
              onClick={() => setPolicy(value)}
            >
              {label}
            </button>
          ))}
        </div>
        <p className="stg-card-note">{POLICY_NOTES[policy]}</p>
      </section>

      <section className="stg-card">
        <h3 className="stg-card-title">Running low means</h3>
        <div className="stg-segmented" role="group" aria-label="Usage left that counts as low">
          {NEAR_PERCENT_CHOICES.map((value) => (
            <button
              key={value}
              type="button"
              className={value === nearPercent ? "stg-seg is-active" : "stg-seg"}
              aria-pressed={value === nearPercent}
              onClick={() => persistFallbacks({ ...settings, nearPercent: value })}
            >
              {value}% left
            </button>
          ))}
        </div>
        <p className="stg-card-note">
          Checked against the tightest window of the runner's plan (5-hour, weekly…), every minute.
        </p>
      </section>

      <ModelSetsSection runners={runners} />

      <section className="stg-card">
        <div className="stg-card-head">
          <h3 className="stg-card-title">Move to, in this order</h3>
          <span className="stg-count">{pairs.length}</span>
        </div>
        <p className="stg-card-note">
          Tried top to bottom. Skipped when its runner is also low or uses the same account.
          {" "}Choose a permission for each handoff, or keep the current one when the next runner supports it.
        </p>
        {pairs.length === 0 ? (
          <p className="rlim-empty">Nothing chosen yet — opman will suggest the runner with the most left.</p>
        ) : (
          <ol className="rlim-list">
            {pairs.map((pair, index) => (
              <li key={`${pair.runner}|${pair.model?.providerID}|${pair.model?.modelID}`} className="rlim-item">
                <span className="rlim-rank">{index + 1}</span>
                <span className="rlim-runner">{runnerLabel(pair.runner)}</span>
                <span className="rlim-model">
                  {pair.model ? shortModelName(pair.model.modelID) : "default model"}
                </span>
                <FallbackPermission runner={pair.runner} permission={pair.permission} index={index}
                  onChange={(permission) => setPairs(pairs.map((entry, at) =>
                    at === index ? { ...entry, permission } : entry))} />
                <span className="rlim-item-actions">
                  <button type="button" className="stg-icon-btn" aria-label="Move up"
                    disabled={index === 0} onClick={() => move(index, -1)}>
                    <ArrowUp size={13} />
                  </button>
                  <button type="button" className="stg-icon-btn" aria-label="Move down"
                    disabled={index === pairs.length - 1} onClick={() => move(index, 1)}>
                    <ArrowDown size={13} />
                  </button>
                  <button type="button" className="stg-icon-btn" aria-label="Remove" onClick={() => remove(index)}>
                    <X size={13} />
                  </button>
                </span>
              </li>
            ))}
          </ol>
        )}
        <FallbackAdder runners={runners} onAdd={add} />
      </section>
    </div>
  );
}
