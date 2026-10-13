/**
 * EngineChip — the composer's single entry point to runner, model, and agent.
 *
 * One chip, three facts, in the order they depend on each other: the runner
 * decides which models and agents exist, so it reads first. Each part is
 * legible on its own, and the whole thing is one target instead of three
 * neighbouring ones that were easy to hit by mistake.
 */
import { useState } from "react";
import { ChevronDown } from "lucide-react";
import type { AgentInfo } from "../api";
import { agentColor, shortModelName, RUNNER_LABELS } from "../prompt-input/helpers";
import { EnginePalette } from "./EnginePalette";
import { useEngineOptions } from "./useEngineOptions";
import { UsageRing } from "./UsageRing";
import { useRunnerUsage } from "../usage/useUsage";

interface Props {
  runner: string;
  availableRunners: string[];
  currentModel: string | null;
  selectedModel: { providerID: string; modelID: string } | null;
  currentAgent: string;
  agents: AgentInfo[];
  disabled: boolean;
  supportedEfforts: string[];
  effort: string | null;
  permission: string;
  onRunnerChange: (runner: string) => void;
  onModelSelected: (modelId: string, providerId: string) => void;
  onAgentChange: (agentId: string) => void;
  onEffortChange: (effort: string | null) => void;
  onPermissionChange: (permission: string) => void;
}

export function EngineChip(props: Props) {
  const [open, setOpen] = useState(false);

  /**
   * Mounted here, not in the palette.
   *
   * This hook is what keeps runner, model and agent consistent with each
   * other, and it can only do that while it is rendered. Inside the palette it
   * ran solely when the picker was open, so the chip could advertise a pairing
   * the runner has never heard of — `Claude · zen/big-pickle` — right up until
   * the user opened the one surface that would have corrected it. The chip is
   * the thing making the claim, so the chip carries the invariant.
   */
  const options = useEngineOptions(
    props.runner,
    props.selectedModel,
    props.currentAgent,
    props.onModelSelected,
    props.onAgentChange,
    props.permission,
    props.onPermissionChange,
  );

  // The chip's usage glance. Loading here (deduplicated and cached for a minute by the
  // store) means the gauge is on screen before anyone opens the palette.
  const { account } = useRunnerUsage(props.runner);
  const runnerName = RUNNER_LABELS[props.runner] || props.runner;

  // What the next prompt will actually run as. An unset agent means "the runner's own
  // default", which is the first agent it lists — so the chip names it rather than
  // writing it back into the session just to have something to show.
  const effectiveAgent = props.currentAgent || options.agents[0]?.id || "";
  const agent = props.agents.find((a) => a.id === effectiveAgent)
    || options.agents.find((a) => a.id === effectiveAgent);
  const agentLabel = agent?.label || effectiveAgent;
  const dot = agentColor(effectiveAgent, agent?.color);
  // The runner's own name for the model ("Sonnet 5.5"), not its id ("sonnet"): an alias id
  // reads like a different model than the one picked.
  const modelLabel = props.currentModel
    ? options.models.find((m) => m.modelId === props.currentModel)?.modelName
      ?? shortModelName(props.currentModel)
    : "Model";

  return (
    <>
      <button
        type="button"
        className="prompt-chip prompt-engine-chip"
        onClick={() => setOpen(true)}
        disabled={props.disabled}
        title="Choose runner, model, agent, effort, and permissions"
        aria-haspopup="dialog"
        aria-expanded={open}
      >
        <span className="engine-chip-runner">
          {runnerName}
          <UsageRing account={account} runnerLabel={runnerName} />
        </span>
        <span className="engine-chip-sep" aria-hidden="true" />
        <span className="engine-chip-model">
          {modelLabel}
        </span>
        {agentLabel && (
          <>
            <span className="engine-chip-sep" aria-hidden="true" />
            <span className="engine-chip-agent">
              <span className="prompt-agent-dot" style={{ backgroundColor: dot }} />
              {agentLabel}
            </span>
          </>
        )}
        <ChevronDown size={9} />
      </button>
      {open && (
        <EnginePalette
          runner={props.runner}
          availableRunners={props.availableRunners}
          selectedModel={props.selectedModel}
          selectedAgent={effectiveAgent}
          supportedEfforts={props.supportedEfforts}
          effort={props.effort}
          permission={props.permission}
          onRunnerChange={props.onRunnerChange}
          onModelSelected={props.onModelSelected}
          onAgentChange={props.onAgentChange}
          onEffortChange={props.onEffortChange}
          onPermissionChange={props.onPermissionChange}
          options={options}
          onClose={() => setOpen(false)}
        />
      )}
    </>
  );
}
