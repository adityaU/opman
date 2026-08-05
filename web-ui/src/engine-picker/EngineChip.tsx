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
import { agentColor, shortModelName } from "../prompt-input/helpers";
import { EnginePalette } from "./EnginePalette";

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

const RUNNER_LABELS: Record<string, string> = {
  opencode: "OpenCode",
  "claude-code": "Claude Code",
  claude: "Claude",
  codex: "Codex",
};

export function EngineChip(props: Props) {
  const [open, setOpen] = useState(false);
  const agent = props.agents.find((a) => a.id === props.currentAgent);
  const agentLabel = agent?.label || props.currentAgent;
  const dot = agentColor(props.currentAgent, agent?.color);

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
        <span className="engine-chip-runner">{RUNNER_LABELS[props.runner] || props.runner}</span>
        <span className="engine-chip-sep" aria-hidden="true" />
        <span className="engine-chip-model">
          {props.currentModel ? shortModelName(props.currentModel) : "Model"}
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
          selectedAgent={props.currentAgent}
          supportedEfforts={props.supportedEfforts}
          effort={props.effort}
          permission={props.permission}
          onRunnerChange={props.onRunnerChange}
          onModelSelected={props.onModelSelected}
          onAgentChange={props.onAgentChange}
          onEffortChange={props.onEffortChange}
          onPermissionChange={props.onPermissionChange}
          onClose={() => setOpen(false)}
        />
      )}
    </>
  );
}
