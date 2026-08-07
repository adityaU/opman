import { apiDelete, apiFetch, apiPost, apiPut } from "./client";

/**
 * `~/.config/opman/mcp.json`, read and written through the backend.
 *
 * Every write reloads the registry in place and broadcasts `mcp_servers_changed` on the
 * existing `/api/events` stream, so a caller refetches on that event rather than polling.
 *
 * Field names match the config document's own spelling — this module is an editor for
 * that file, not a separate representation of it.
 */

/**
 * Window event re-broadcast from the `mcp_servers_changed` SSE event.
 *
 * The app already holds one `/api/events` stream; the settings page listens for this
 * instead of opening a second one or polling.
 */
export const MCP_SERVERS_CHANGED = "opman:mcp-servers-changed";

export type McpTransport = "stdio" | "http" | "sse";

/** `none` — no credential. `static` — one already in `headers`. `oauth` — opman mints it. */
export type McpAuth = "none" | "static" | "oauth";

export interface McpServer {
  name: string;
  transport: McpTransport;
  auth: McpAuth;
  enabled: boolean;
  /** opman ships this one. Toggleable, but removing the entry only restores the default. */
  builtin: boolean;
  url?: string;
  command?: string;
  args: string[];
  /** Names only — an `env` value is a credential, so it never leaves opman. */
  envNames: string[];
  /** Names only, for the same reason: a `static` credential lives in a header. */
  headerNames: string[];
  timeoutSecs?: number;
  /** The credential is minted by opman and never reaches the runner. */
  proxied: boolean;
  /** Runners this server is offered to. Empty means all of them. */
  runners: string[];
  /** OpenCode is handed its config once at spawn, so a change lands there on restart. */
  needsOpencodeRestart: boolean;
  /** A usable OAuth credential is stored. Only meaningful when `auth` is `oauth`. */
  authenticated: boolean;
}

/**
 * A server as the settings page submits it — a patch, not a replacement.
 *
 * An omitted field is left as it stands. That is what makes the form safe: it is never
 * shown an `env` or `headers` value, so it cannot resend one, and a body that overwrote
 * everything it happened to know would delete the credentials it never saw.
 *
 * An empty array or an explicit `null` is a change; `undefined` is silence.
 */
export interface McpServerDraft {
  type?: McpTransport;
  command?: string;
  args?: string[];
  url?: string;
  auth?: McpAuth;
  runners?: string[];
  timeoutSecs?: number | null;
  /** Variables to add or overwrite — the only way to change a value. */
  envSet?: Record<string, string>;
  envRemove?: string[];
  headersSet?: Record<string, string>;
  headersRemove?: string[];
}

export interface McpLoginStart {
  /** Open in a new tab. Its final redirect is on loopback and will not load remotely. */
  authorizeUrl: string;
  /** The loopback address the browser was told to return to. */
  redirectUri: string;
}

const server = (name: string) => `/mcp/servers/${encodeURIComponent(name)}`;

/**
 * The declared servers, plus the built-ins with no entry of their own.
 *
 * Every list field is defaulted on the way in. The rows render them directly — spreading
 * `args` into a command line, joining `runners` — so one absent key from an older backend
 * would take the whole section down rather than degrade it.
 */
export async function fetchMcpServers(): Promise<McpServer[]> {
  const raw = await apiFetch<Partial<McpServer>[]>("/mcp/servers");
  return raw.map((server) => ({
    transport: "stdio",
    auth: "none",
    enabled: true,
    builtin: false,
    proxied: false,
    needsOpencodeRestart: true,
    authenticated: false,
    ...server,
    name: server.name ?? "",
    args: server.args ?? [],
    envNames: server.envNames ?? [],
    headerNames: server.headerNames ?? [],
    runners: server.runners ?? [],
  }));
}

export async function saveMcpServer(name: string, draft: McpServerDraft): Promise<void> {
  await apiPut(server(name), draft);
}

/**
 * Turn a server on or off.
 *
 * Works for a built-in the user has never written an entry for: the backend records a
 * patch rather than a full definition, so opman's own launch command is never copied
 * into user config where it would then go stale.
 */
export async function setMcpServerEnabled(name: string, enabled: boolean): Promise<void> {
  await apiPost(`${server(name)}/enabled`, { enabled });
}

export async function deleteMcpServer(name: string): Promise<void> {
  await apiDelete(server(name));
}

/** Begin an OAuth login and get the URL the browser must visit. */
export async function startMcpLogin(name: string): Promise<McpLoginStart> {
  return apiPost<McpLoginStart>(`${server(name)}/login`);
}

/**
 * Hand the pending login the callback the browser could not deliver.
 *
 * opman's loopback redirect is unreachable from a remote browser, so the user pastes the
 * URL their browser ended up at. Only its query is used; the address it is delivered to
 * comes from the pending flow.
 */
export async function finishMcpLogin(name: string, url: string): Promise<void> {
  await apiPost(`${server(name)}/login/finish`, { url });
}

export async function logoutMcpServer(name: string): Promise<void> {
  await apiPost(`${server(name)}/logout`);
}

/* ── Tool catalog ───────────────────────────────────────────────────────
   Everything above is an editor for `mcp.json` and can only report what the
   user wrote. This is the other question: given that entry, what does an
   agent actually end up with. The backend answers it by launching the server
   and asking, so a reply costs a process and takes a moment. */

/** One tool, verbatim as its server described it. Schemas pass through whole. */
export interface McpTool {
  name: string;
  title?: string;
  description?: string;
  inputSchema?: import("../settings-page/mcp/schema").JsonSchema;
  outputSchema?: import("../settings-page/mcp/schema").JsonSchema;
  annotations?: Record<string, unknown>;
}

/**
 * The outcome of one probe.
 *
 * A server that will not start is a `failed`, not a rejected request: which server broke
 * and how is exactly what the panel shows, and an HTTP status cannot carry it.
 */
export type McpCatalog =
  | { status: "listed"; server?: { name: string; version?: string }; tools: McpTool[] }
  /** opman declined to launch it — an unmet presence condition, or an unresolvable argument. */
  | { status: "unavailable"; reason: string }
  /** It was launched and did not answer. */
  | { status: "failed"; reason: string };

/** Launch one declared server and read its `tools/list`. */
export async function fetchMcpServerTools(name: string): Promise<McpCatalog> {
  const catalog = await apiFetch<McpCatalog>(`${server(name)}/tools`);
  if (catalog.status !== "listed") return catalog;
  return { ...catalog, tools: catalog.tools ?? [] };
}
