/**
 * EnginePalette — one searchable surface for runner, model, and agent.
 *
 * These three were three separate controls: a modal, an anchored popover, and
 * a second modal, each with its own idea of what a choice looks like. They are
 * one decision — which engine answers the next turn — and changing the runner
 * silently invalidated the other two. Here the dependency is visible: pick a
 * runner and the lists below reload for it, in place, without closing.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Search, Check, Cpu, Bot, Zap } from "lucide-react";
import type { AgentInfo } from "../api";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import type { EngineOptions, ModelOption } from "./useEngineOptions";
import { EngineSettingsRow } from "./EngineSettingsRow";

interface Props {
  runner: string;
  availableRunners: string[];
  selectedModel: { providerID: string; modelID: string } | null;
  selectedAgent: string;
  supportedEfforts: string[];
  effort: string | null;
  permission: string;
  onRunnerChange: (runner: string) => void;
  onModelSelected: (modelId: string, providerId: string) => void;
  onAgentChange: (agentId: string) => void;
  onEffortChange: (effort: string | null) => void;
  onPermissionChange: (permission: string) => void;
  onClose: () => void;
  /**
   * Supplied by `EngineChip` rather than read here. The hook that produces
   * these also repairs an impossible runner/model pair, and a repair that only
   * runs while this dialog is mounted is a repair that never runs when it
   * matters — see useEngineOptions.
   */
  options: EngineOptions;
}

type Row =
  | { kind: "runner"; id: string; label: string; selected: boolean }
  | { kind: "model"; id: string; label: string; hint: string; selected: boolean; model: ModelOption }
  | { kind: "agent"; id: string; label: string; hint: string; selected: boolean; agent: AgentInfo };

const RUNNER_LABELS: Record<string, string> = {
  opencode: "OpenCode",
  "claude-code": "Claude Code",
  claude: "Claude",
  codex: "Codex",
};

export function EnginePalette(props: Props) {
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  useEscape(props.onClose);
  useFocusTrap(panelRef);
  useEffect(() => {
    // On touch devices, focusing the search input on open pops the software
    // keyboard over the very list the user came to tap. Park focus on the
    // panel instead (the trap has already focused the input — take it back);
    // tapping the field still summons the keyboard when search is wanted.
    if (window.matchMedia?.("(pointer: coarse)").matches) {
      inputRef.current?.blur();
      panelRef.current?.focus();
    } else {
      inputRef.current?.focus();
    }
  }, []);

  const { models, agents, permissionModes, loading } = props.options;

  const rows = useMemo<Row[]>(() => {
    const q = query.trim().toLowerCase();
    const match = (...fields: string[]) =>
      !q || fields.some((field) => field.toLowerCase().includes(q));

    const runnerRows: Row[] = props.availableRunners
      .filter((id) => match(id, RUNNER_LABELS[id] || id))
      .map((id) => ({
        kind: "runner" as const,
        id,
        label: RUNNER_LABELS[id] || id,
        selected: id === props.runner,
      }));

    const modelRows: Row[] = models
      .filter((m) => match(m.modelName, m.modelId, m.providerName))
      .map((m) => ({
        kind: "model" as const,
        id: `${m.providerId}/${m.modelId}`,
        label: m.modelName,
        hint: m.providerName,
        selected: props.selectedModel?.modelID === m.modelId
          && props.selectedModel?.providerID === m.providerId,
        model: m,
      }));

    const agentRows: Row[] = agents
      .filter((a) => !a.hidden && match(a.label || a.id, a.id, a.description || ""))
      .map((a) => ({
        kind: "agent" as const,
        id: a.id,
        label: a.label || a.id,
        hint: a.description || "",
        selected: a.id === props.selectedAgent,
        agent: a,
      }));

    return [...runnerRows, ...modelRows, ...agentRows];
  }, [query, props.availableRunners, props.runner, models, agents, props.selectedModel, props.selectedAgent]);

  useEffect(() => { setCursor(0); }, [query, props.runner]);

  const choose = (row: Row) => {
    if (row.kind === "runner") {
      // Stay open: the lists below are about to change, and the point of one
      // surface is that the consequence is visible where the cause was.
      props.onRunnerChange(row.id);
      setQuery("");
      return;
    }
    if (row.kind === "model") props.onModelSelected(row.model.modelId, row.model.providerId);
    else props.onAgentChange(row.agent.id);
    props.onClose();
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setCursor((c) => Math.min(rows.length - 1, c + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setCursor((c) => Math.max(0, c - 1));
    } else if (event.key === "Enter" && rows[cursor]) {
      event.preventDefault();
      choose(rows[cursor]);
    }
  };

  useEffect(() => {
    const active = listRef.current?.querySelector<HTMLElement>(".engine-row.is-cursor");
    active?.scrollIntoView({ block: "nearest" });
  }, [cursor]);

  let lastKind = "";

  return createPortal(
    <div className="engine-palette-backdrop modal-backdrop" onMouseDown={props.onClose}>
      <div
        className="engine-palette modal-dialog-surface"
        role="dialog"
        aria-label="Choose runner, model, and agent"
        tabIndex={-1}
        ref={panelRef}
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={onKeyDown}
      >
        <div className="engine-palette-search">
          <Search size={14} />
          <input
            ref={inputRef}
            className="engine-palette-input"
            value={query}
            placeholder="Search runners, models, agents…"
            onChange={(event) => setQuery(event.target.value)}
            aria-label="Search runners, models and agents"
          />
        </div>

        <div className="engine-palette-list" ref={listRef} role="listbox">
          {rows.length === 0 && (
            <p className="engine-palette-empty">
              {loading ? `Loading ${props.runner} options…` : `Nothing matches “${query}”.`}
            </p>
          )}
          {rows.map((row, index) => {
            const header = row.kind !== lastKind ? row.kind : null;
            lastKind = row.kind;
            return (
              <div key={`${row.kind}-${row.id}`}>
                {header && (
                  <div className="engine-group">
                    {header === "runner" && <><Zap size={11} /> Runner</>}
                    {header === "model" && <><Cpu size={11} /> Model · {props.runner}</>}
                    {header === "agent" && <><Bot size={11} /> Agent · {props.runner}</>}
                  </div>
                )}
                <button
                  type="button"
                  role="option"
                  aria-selected={row.selected}
                  className={`engine-row${row.selected ? " is-selected" : ""}${index === cursor ? " is-cursor" : ""}`}
                  onMouseEnter={() => setCursor(index)}
                  onClick={() => choose(row)}
                >
                  <span className="engine-row-label">{row.label}</span>
                  {"hint" in row && row.hint && <span className="engine-row-hint">{row.hint}</span>}
                  {row.selected && <Check size={13} className="engine-row-check" />}
                </button>
              </div>
            );
          })}
        </div>

        <EngineSettingsRow
          runner={props.runner}
          permissionModes={permissionModes}
          supportedEfforts={props.supportedEfforts}
          effort={props.effort}
          permission={props.permission}
          onEffortChange={props.onEffortChange}
          onPermissionChange={props.onPermissionChange}
        />
      </div>
    </div>,
    document.body,
  );
}
