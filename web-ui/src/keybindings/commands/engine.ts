import type { CommandDef } from "../types";

/** Runner, model, agent and the per-runner execution settings. */
export const ENGINE_COMMANDS: readonly CommandDef[] = [
  { id: "engine.palette", title: "Choose Engine…", category: "Engine", label: "engine" },
  { id: "engine.runner", title: "Choose Runner…", category: "Engine", label: "runner" },
  { id: "engine.model", title: "Choose Model…", category: "Engine", label: "model" },
  { id: "engine.agent", title: "Choose Agent…", category: "Engine", label: "agent" },
  { id: "engine.effort", title: "Set Reasoning Effort…", category: "Engine", when: "runnerHasEffort", label: "effort" },
  { id: "engine.permissionMode", title: "Set Permission Mode…", category: "Engine", label: "permissions" },
  { id: "engine.refreshProviders", title: "Refresh Providers", category: "Engine", paletteOnly: true },
];

/** The assistant suite — everything above an individual session. */
export const ASSISTANT_COMMANDS: readonly CommandDef[] = [
  { id: "assistant.routines", title: "Routines", category: "Assistant", label: "routines" },
  { id: "assistant.instructions", title: "Session Instructions", category: "Assistant", label: "instructions" },
  { id: "assistant.autonomy", title: "Autonomy Mode", category: "Assistant", label: "autonomy" },
  { id: "assistant.notifications", title: "Notification Preferences", category: "Assistant", label: "notifications" },
  { id: "assistant.autoOpen", title: "Auto-Open Settings", category: "Assistant", label: "auto-open" },
];
