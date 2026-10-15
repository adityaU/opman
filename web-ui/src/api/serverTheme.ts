/**
 * Each opman server keeps its own theme. A pane showing a remote server's content is
 * that server's page, so it paints with that server's theme — these let settings read
 * and set it for any server, not only the one the page is on.
 */
import { serverRequest } from "./client";
import type { ThemePair } from "./state";

export function fetchServerTheme(serverId: string): Promise<ThemePair> {
  return serverRequest<ThemePair>(serverId, "GET", "/theme");
}

export function setServerTheme(serverId: string, name: string): Promise<ThemePair> {
  return serverRequest<ThemePair>(serverId, "POST", "/theme/switch", { name });
}
