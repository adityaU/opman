import type { WorkspaceSnapshot } from "../api";

/** Human-readable summary of what a snapshot configures. */
export function describeSnapshot(ws: WorkspaceSnapshot): string {
  const parts: string[] = [];
  const panels = ws.panels;
  if (panels.sidebar) parts.push("Sidebar");
  if (panels.terminal) parts.push("Terminal");
  if (panels.editor) parts.push("Editor");
  if (panels.git) parts.push("Git");
  if (ws.open_files.length > 0)
    parts.push(`${ws.open_files.length} file${ws.open_files.length > 1 ? "s" : ""}`);
  if (ws.git_branch) parts.push(`branch: ${ws.git_branch}`);
  if (ws.is_recipe) parts.push("recipe");
  return parts.join(" + ") || "Empty";
}

export function formatDate(iso: string): string {
  if (!iso) return "";
  try {
    const d = new Date(iso);
    return d.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}
