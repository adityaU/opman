import { switchProject, fetchAppState } from "./api";
import type { HandlerDeps } from "./chatLayoutHandlers";

/**
 * Handlers for moving between sessions and projects.
 *
 * Split from `chatLayoutHandlers` on the seam that already existed: those act on the
 * conversation in front of you, these change which conversation that is.
 */

export function createHandleSelectSession(deps: HandlerDeps) {
  return (sessionId: string, projectIdx: number) => {
    if (!deps.appState) return;
    // Close mobile sidebar without history.back() so the subsequent
    // pushState for the new session URL isn't undone by the async back().
    deps.closeMobileSidebarSilent();
    // URL is the single source of truth — this triggers beginSessionSwitch + API calls
    deps.setUrlSession(sessionId, projectIdx);
    deps.setSelectedModel(null);
    deps.setSelectedAgent("");
    deps.clearRunnerChoice();
  };
}

export function createHandleNewSession(deps: HandlerDeps) {
  return async () => {
    if (!deps.appState) return;
    try {
      const projectIdx = deps.activeProjectIndex;
      deps.setUrlSession(null, projectIdx);
      deps.setSelectedModel(null);
      deps.setSelectedModel(null);
      deps.setSelectedAgent("");

      deps.addToast("New session created", "success");
    } catch {
      deps.addToast("Failed to create session", "error");
    }
  };
}

export function createHandleSwitchProject(deps: HandlerDeps) {
  return async (index: number) => {
    try {
      await switchProject(index);
      // Fetch fresh state to discover the new project's active session
      const freshState = await fetchAppState();
      const proj = freshState.projects[index];
      const newSid = proj?.active_session ?? null;
      if (newSid) {
        // URL is the single source of truth — triggers beginSessionSwitch + API calls
        deps.setUrlSession(newSid, index);
      }
      deps.setSelectedModel(null);
      deps.setSelectedAgent("");

    } catch {
      deps.addToast("Failed to switch project", "error");
    }
  };
}

export function createHandleModelSelected(deps: HandlerDeps) {
  return (modelId: string, providerId: string) => {
    deps.blockSessionAdoption();
    deps.setSelectedModel({ providerID: providerId, modelID: modelId });
    deps.addToast(`Model switched to ${modelId}`, "success");
  };
}

/**
 * Runner a brand-new session should be created with.
 *
 * `appState.backend` names the CLI opman wraps, not a runner — both claude
 * engines report "claude-code" — so it can never identify one. `default_runner`
 * is the server's own answer; trust it and nothing else.
 */
export function defaultRunner(appState: any): string {
  return appState?.default_runner || "opencode";
}
