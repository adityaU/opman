import { serverBase } from "../api/base";
import type { ServerInfo, ServerStatus } from "../api/servers";

/**
 * How a server's reachability reads, and where choosing it goes.
 *
 * The dot is never the only signal: every place that shows one also puts the label in its
 * accessible name or title, because "unreachable" and "sign-in failed" differ only in hue.
 */

export const STATUS_LABEL: Readonly<Record<ServerStatus, string>> = {
  ok: "Connected",
  unreachable: "Unreachable",
  auth_failed: "Sign-in failed",
};

export function ServerStatusDot({ status }: { readonly status: ServerStatus }) {
  return <span className={`srv-dot is-${status}`} aria-hidden="true" />;
}

/** Host and port only — the scheme and path add width, not information. */
export function serverHost(server: ServerInfo): string {
  if (!server.url) return "This machine";
  try {
    return new URL(server.url).host || server.url;
  } catch {
    return server.url;
  }
}

/** Open a server's instance. A full navigation: one page load is one server. */
export function switchToServer(id: string): void {
  window.location.assign(`${serverBase(id)}/`);
}
