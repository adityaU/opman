import type { CommandDef } from "../types";

/** Runner, model, agent and the per-runner execution settings. */
export const ENGINE_COMMANDS: readonly CommandDef[] = [
  { id: "engine.palette", title: "Choose Engine…", category: "Engine", label: "engine", slash: { name: "engine", where: "opman" } },
  { id: "engine.runner", title: "Choose Runner…", category: "Engine", label: "runner" },
  { id: "engine.model", title: "Choose Model…", category: "Engine", label: "model", slash: { name: "model", where: "opman" } },
  { id: "engine.agent", title: "Choose Agent…", category: "Engine", label: "agent", slash: { name: "agent", where: "opman" } },
  { id: "engine.effort", title: "Set Reasoning Effort…", category: "Engine", when: "runnerHasEffort", label: "effort" },
  { id: "engine.permissionMode", title: "Set Permission Mode…", category: "Engine", label: "permissions" },
  { id: "engine.refreshProviders", title: "Refresh Providers", category: "Engine", paletteOnly: true },
];

/** The assistant suite — everything above an individual session. */
export const ASSISTANT_COMMANDS: readonly CommandDef[] = [
  { id: "assistant.routines", title: "Routines", category: "Assistant", label: "routines", slash: { name: "routines", where: "opman" } },
  { id: "assistant.instructions", title: "Session Instructions", category: "Assistant", label: "instructions" },
  { id: "assistant.memories", title: "All Session Instructions", category: "Assistant", label: "memories", slash: { name: "memory", where: "opman" } },
  { id: "assistant.autonomy", title: "Autonomy Mode", category: "Assistant", label: "autonomy", slash: { name: "autonomy", where: "opman" } },
  { id: "assistant.notifications", title: "Notification Preferences", category: "Assistant", label: "notifications", slash: { name: "notification-prefs", where: "opman" } },
  { id: "assistant.autoOpen", title: "Auto-Open Settings", category: "Assistant", label: "auto-open", slash: { name: "auto-open", where: "opman" } },
];
