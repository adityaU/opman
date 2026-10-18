import { ptyWrite, spawnPty } from "../../api/pty";
import { queueWidgetOpen } from "../../workspace/pendingOpen";
import type { AcpAgent } from "../../api/acp";

/**
 * Open a terminal pane that logs an agent in.
 *
 * A shell is started under a fresh id and given the login command, then a terminal widget
 * pointing at it is queued for the workspace. Using a shell rather than running the command
 * directly keeps the pane useful after the login ends.
 */
export async function startLogin(
  agent: Pick<AcpAgent, "id" | "displayName">,
  command: string,
  projectPath: string,
): Promise<void> {
  const ptyId = `login-${agent.id}-${Date.now().toString(36)}`;
  await spawnPty("shell", ptyId, 24, 80, {
    project: projectPath,
    label: `Login: ${agent.displayName}`,
  });
  await ptyWrite(ptyId, `${command}\n`);
  queueWidgetOpen({ kind: "terminal", projectPath, ptyId });
}
