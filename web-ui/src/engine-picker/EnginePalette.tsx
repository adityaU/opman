/**
 * EnginePalette — one searchable surface for runner, model, and agent.
 *
 * These three were three separate controls: a modal, an anchored popover, and
 * a second modal, each with its own idea of what a choice looks like. They are
 * one decision — which engine answers the next turn — and changing the runner
 * silently invalidated the other two. Here the dependency is visible: pick a
 * runner and the lists below reload for it, in place, without closing.
 *
 * Each runner row also shows how much its account has left, because "which
 * runner" is partly "which one can still take the turn". Usage is read from a
 * shared store (usage/usageStore.ts) that this surface refreshes on open.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Search, Cpu, Bot, Zap } from "lucide-react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { useUsage } from "../usage/useUsage";
import { accountForRunner } from "../usage/format";
import type { EngineOptions } from "./useEngineOptions";
import { EngineSettingsRow } from "./EngineSettingsRow";
import { startRunner } from "./startRunner";
import { buildRows, runnerLabel, type Row } from "./paletteRows";
import { ChoiceRow, GroupHeader, RowsSkeleton, RunnerRow, UsageHeaderStatus } from "./EngineRows";

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

/** A clock that moves while the palette is open, so "resets in 4m" does not freeze. */
function useNow(intervalMs = 30_000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(timer);
  }, [intervalMs]);
  return now;
}

export function EnginePalette(props: Props) {
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const [startingRunner, setStartingRunner] = useState<string | null>(null);
  const [runnerError, setRunnerError] = useState<string | null>(null);
  // Opening the palette is the moment usage matters; the store makes this a no-op
  // when the chip (or another composer) fetched it within the last minute.
  const usage = useUsage();
  const now = useNow();

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

  const { models, agents, permissionModes, loadingModels, loadingAgents } = props.options;

  const rows = useMemo<Row[]>(() => buildRows({
    query,
    runner: props.runner,
    availableRunners: props.availableRunners,
    models,
    agents,
    selectedModel: props.selectedModel,
    selectedAgent: props.selectedAgent,
  }), [query, props.availableRunners, props.runner, models, agents, props.selectedModel, props.selectedAgent]);

  useEffect(() => { setCursor(0); }, [query, props.runner]);

  const choose = async (row: Row) => {
    if (row.kind === "runner") {
      // Do not publish the new runner until its server is ready. Publishing first lets the
      // dependent provider/agent requests race this startup call and cache an empty 503.
      if (startingRunner) return;
      setRunnerError(null);
      setStartingRunner(row.id);
      try {
        await startRunner(row.id);
        // Stay open: the lists below are about to change, and the point of one
        // surface is that the consequence is visible where the cause was.
        props.onRunnerChange(row.id);
        setQuery("");
      } catch (error: unknown) {
        setRunnerError(error instanceof Error ? error.message : `Could not start ${row.label}`);
      } finally {
        setStartingRunner(null);
      }
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
    } else if (event.key === "Enter" && rows[cursor] && ownsEnter(event.target)) {
      event.preventDefault();
      void choose(rows[cursor]);
    }
  };

  // Enter picks the cursor row from the search field, the panel, or a row — but not from
  // the refresh button or the footer controls, which have their own Enter.
  const ownsEnter = (target: EventTarget) =>
    target === inputRef.current || target === panelRef.current
    || (target instanceof HTMLElement && target.classList.contains("engine-row"));

  useEffect(() => {
    const active = listRef.current?.querySelector<HTMLElement>(".engine-row.is-cursor");
    active?.scrollIntoView?.({ block: "nearest" });
  }, [cursor]);

  const shell = (row: Row, index: number) => ({
    index,
    cursor,
    onHover: setCursor,
    onChoose: () => void choose(row),
  });

  const indexed = rows.map((row, index) => ({ row, index }));
  const runnerRows = indexed.filter(({ row }) => row.kind === "runner");
  const modelRows = indexed.filter(({ row }) => row.kind === "model");
  const agentRows = indexed.filter(({ row }) => row.kind === "agent");
  const label = runnerLabel(props.runner);
  // Skeletons stand in for a list that is coming, never for a search that matched nothing.
  const modelsPending = loadingModels && models.length === 0 && !query;
  const agentsPending = loadingAgents && agents.length === 0 && !query;

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
          <Search size={14} aria-hidden="true" />
          <input
            ref={inputRef}
            className="engine-palette-input"
            value={query}
            placeholder="Search runners, models, agents…"
            onChange={(event) => setQuery(event.target.value)}
            aria-label="Search runners, models and agents"
          />
        </div>

        <div className="engine-palette-list" ref={listRef} role="listbox" aria-label="Runners, models and agents">
          {runnerError && <p className="engine-palette-notice" role="alert">{runnerError}</p>}
          {startingRunner && (
            <span className="skel-sr-only" role="status">Starting {runnerLabel(startingRunner)}…</span>
          )}

          {runnerRows.length > 0 && (
            <GroupHeader icon={<Zap size={11} aria-hidden="true" />} title="Runner">
              <UsageHeaderStatus usage={usage} now={now} />
            </GroupHeader>
          )}
          {runnerRows.map(({ row, index }) => row.kind === "runner" && (
            <RunnerRow
              key={row.id}
              {...shell(row, index)}
              row={row}
              account={accountForRunner(usage.report, row.id)}
              usage={usage}
              starting={startingRunner === row.id}
              now={now}
            />
          ))}

          {(modelRows.length > 0 || modelsPending) && (
            <GroupHeader icon={<Cpu size={11} aria-hidden="true" />} title="Model" context={label} />
          )}
          {modelsPending && <RowsSkeleton kind="model" label={`Loading ${label} models`} />}
          {modelRows.map(({ row, index }) => row.kind === "model" && <ChoiceRow key={row.id} {...shell(row, index)} row={row} />)}

          {(agentRows.length > 0 || agentsPending) && (
            <GroupHeader icon={<Bot size={11} aria-hidden="true" />} title="Agent" context={label} />
          )}
          {agentsPending && <RowsSkeleton kind="agent" label={`Loading ${label} agents`} />}
          {agentRows.map(({ row, index }) => row.kind === "agent" && <ChoiceRow key={row.id} {...shell(row, index)} row={row} />)}

          {rows.length === 0 && !modelsPending && !agentsPending && (
            <p className="engine-palette-empty">Nothing matches “{query}”.</p>
          )}
        </div>

        <EngineSettingsRow
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
