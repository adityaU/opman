import type { NativeTool, NativeToolsInfo, NativeToolsSetting } from "../../api/acp";

/**
 * The arithmetic behind the native-tools switches, kept apart from the markup so the three
 * states of the master switch can be tested without rendering anything.
 */

export type MasterState = "all" | "none" | "some";

export function masterState(tools: readonly NativeTool[]): MasterState {
  const on = tools.filter((tool) => tool.enabled).length;
  if (on === tools.length) return "all";
  return on === 0 ? "none" : "some";
}

/** What the master switch asks for: everything on, unless everything already is. */
export function afterMasterClick(tools: readonly NativeTool[]): NativeToolsSetting {
  return masterState(tools) === "all" ? "off" : "all";
}

/** The setting that results from flipping one tool. */
export function afterToolClick(tools: readonly NativeTool[], id: string): NativeToolsSetting {
  const disabled = tools
    .filter((tool) => (tool.id === id ? tool.enabled : !tool.enabled))
    .map((tool) => tool.id);
  if (disabled.length === 0) return "all";
  return disabled.length === tools.length ? "off" : { disabled };
}

/** The view a setting will produce, so the page can show it before the server answers. */
export function applySetting(info: NativeToolsInfo, setting: NativeToolsSetting): NativeToolsInfo {
  const off = (id: string) =>
    setting === "off" || (typeof setting === "object" && setting.disabled.includes(id));
  return {
    ...info,
    setting,
    tools: info.tools.map((tool) => ({ ...tool, enabled: !off(tool.id) })),
  };
}
