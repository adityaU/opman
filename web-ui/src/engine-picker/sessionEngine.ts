/**
 * Reading a session's engine configuration off the row its runner reports.
 *
 * Shared by the shell composer and the workspace panes, which both have to answer the
 * same question — "what is this session set to?" — and previously each answered it from
 * their own local memory instead of from the session.
 */
import type { EngineChoices, SessionInfo } from "../api";
import { DEFAULT_PERMISSION } from "../api";
import type { PaneEngine } from "../workspace/types";

/** The provider catalogue, in the shape `useProviders` returns it. */
export interface ProviderCatalogue {
  all: Array<{ id: string; models: Record<string, unknown> }>;
}

export interface ModelRef {
  providerID: string;
  modelID: string;
}

/**
 * Resolve a bare model id to the provider that offers it.
 *
 * Runners store a model as one string because that is all their command line takes; the
 * composer needs the provider too. An id no provider claims resolves to null rather than
 * to a guess — the engine palette then repairs it to the runner's own default, which is
 * the one place that repair belongs.
 */
export function resolveModel(
  modelId: string | undefined,
  providers: ProviderCatalogue,
): ModelRef | null {
  if (!modelId) return null;
  for (const provider of providers.all) {
    if (provider.models && modelId in provider.models) {
      return { providerID: provider.id, modelID: modelId };
    }
  }
  return null;
}

/**
 * A session's configuration as a full engine, falling back field by field.
 *
 * Per field rather than all-or-nothing: a session that has had only its model chosen
 * should show that model and inherit the rest, not be treated as unconfigured.
 */
export function engineFromSession(
  session: SessionInfo | undefined,
  runner: string,
  providers: ProviderCatalogue,
  fallback: PaneEngine,
): PaneEngine {
  // The configuration describes the runner it was made in, so it means nothing under
  // another one — the model would come from a catalogue this runner has never seen.
  const engine: EngineChoices | undefined =
    session && (!session.runner || session.runner === runner) ? session.engine : undefined;
  if (!engine) return { ...fallback, runner };
  return {
    runner,
    model: resolveModel(engine.model, providers) ?? fallback.model,
    agent: engine.agent ?? fallback.agent,
    effort: engine.effort ?? fallback.effort,
    permission: engine.permissionMode ?? fallback.permission ?? DEFAULT_PERMISSION,
  };
}
