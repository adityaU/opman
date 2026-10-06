/**
 * The opman servers this frontend can reach: home plus any configured remotes.
 *
 * Always home's own `/api/servers`, never `BASE`-relative — the list lives on home, and a
 * remote instance (`/remote/<id>/`) shows and edits the same list. Credentials go in and
 * never come back: `ServerInfo` carries no username or password.
 *
 * For components, `useServers()` in `hooks/useServers.ts` holds one cached copy of the
 * list shared by every caller:
 *
 *   const { servers, current, multi, refresh } = useServers();
 *   // servers: ServerInfo[] (home first), current: this instance's entry,
 *   // multi: more than one server exists — group project pickers by server only then.
 *
 * Contract: docs/multi-server-device-browser.md.
 */
import { homeRequest } from "./client";

export type ServerStatus = "ok" | "unreachable" | "auth_failed";

export interface ServerInfo {
  /** `"home"` or a short slug derived from the name. */
  readonly id: string;
  readonly name: string;
  /** The remote's own URL; `null` for home. */
  readonly url: string | null;
  /** `""` for home, `"/remote/<id>"` otherwise. */
  readonly base: string;
  readonly status: ServerStatus;
}

export interface NewServer {
  readonly name: string;
  readonly url: string;
  readonly username?: string;
  readonly password?: string;
}

/** A patch: an absent field is left alone, an absent password keeps the stored one. */
export interface ServerPatch {
  readonly name?: string;
  readonly url?: string;
  readonly username?: string;
  readonly password?: string;
}

export function listServers(): Promise<ServerInfo[]> {
  return homeRequest<ServerInfo[]>("GET", "/servers");
}

/** Add a remote. The backend logs in to validate it and answers 400 with a reason. */
export function addServer(body: NewServer): Promise<ServerInfo> {
  return homeRequest<ServerInfo>("POST", "/servers", body);
}

export function updateServer(id: string, body: ServerPatch): Promise<ServerInfo> {
  return homeRequest<ServerInfo>("PATCH", `/servers/${encodeURIComponent(id)}`, body);
}

export function removeServer(id: string): Promise<void> {
  return homeRequest("DELETE", `/servers/${encodeURIComponent(id)}`);
}
