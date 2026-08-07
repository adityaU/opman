import React, { useCallback, useMemo, useState } from "react";
import type { McpAuth, McpServer, McpServerDraft, McpTransport } from "../../api/mcp";
import { NO_EDITS, SecretEditor, type SecretEdits } from "./SecretEditor";

/**
 * Declare or edit one server.
 *
 * Everything shown is round-trippable — the form only submits fields it was actually given
 * values for, and the two that carry credentials go through [`SecretEditor`], which edits
 * them by name. A built-in is a special case: an entry naming one *patches* opman's own
 * definition, so offering to rewrite its launch command would be offering a lie.
 */

const TRANSPORTS: readonly McpTransport[] = ["stdio", "http", "sse"];
const AUTHS: readonly McpAuth[] = ["none", "static", "oauth"];

const AUTH_NOTE: Readonly<Record<McpAuth, string>> = {
  none: "No credential. Handed straight to each runner.",
  static: "A credential already in the headers. Fronted by opman's proxy so it never reaches a runner's argv or environment.",
  oauth: "opman is the OAuth client and mints a token per request. Needs one log-in here.",
};

export interface ServerFormProps {
  /** Absent when declaring a new server. */
  readonly server?: McpServer;
  /** Runner slots on offer, so an ACP agent can be scoped like any other. */
  readonly runners: readonly string[];
  readonly saving: boolean;
  readonly onSubmit: (name: string, draft: McpServerDraft) => Promise<boolean>;
  readonly onCancel: () => void;
}

function initialTransport(server?: McpServer): McpTransport {
  return server?.transport ?? "stdio";
}

export function ServerForm({ server, runners, saving, onSubmit, onCancel }: ServerFormProps) {
  const patchOnly = server?.builtin ?? false;
  const [name, setName] = useState(server?.name ?? "");
  const [transport, setTransport] = useState<McpTransport>(initialTransport(server));
  const [command, setCommand] = useState(server?.command ?? "");
  const [args, setArgs] = useState((server?.args ?? []).join("\n"));
  const [url, setUrl] = useState(server?.url ?? "");
  const [auth, setAuth] = useState<McpAuth>(server?.auth ?? "none");
  const [scoped, setScoped] = useState<readonly string[]>(server?.runners ?? []);
  const [timeoutSecs, setTimeoutSecs] = useState(
    server?.timeoutSecs != null ? String(server.timeoutSecs) : "",
  );
  const [env, setEnv] = useState<SecretEdits>(NO_EDITS);
  const [headers, setHeaders] = useState<SecretEdits>(NO_EDITS);
  const [problem, setProblem] = useState<string>();

  const remote = transport !== "stdio";

  const invalid = useMemo(() => {
    if (!name.trim()) return "A name is required.";
    if (patchOnly) return undefined;
    if (remote && !url.trim()) return "A remote server needs a URL.";
    if (!remote && !command.trim()) return "A stdio server needs a command.";
    if (timeoutSecs && !/^\d+$/.test(timeoutSecs.trim())) return "The timeout must be whole seconds.";
    return undefined;
  }, [name, patchOnly, remote, url, command, timeoutSecs]);

  const toggleRunner = (runner: string) =>
    setScoped((current) =>
      current.includes(runner)
        ? current.filter((entry) => entry !== runner)
        : [...current, runner],
    );

  const submit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (invalid) {
        setProblem(invalid);
        return;
      }
      setProblem(undefined);
      const draft: McpServerDraft = {
        runners: [...scoped],
        timeoutSecs: timeoutSecs.trim() ? Number(timeoutSecs.trim()) : null,
        envSet: env.set,
        envRemove: [...env.remove],
      };
      if (!patchOnly) {
        draft.type = transport;
        draft.auth = auth;
        // Only the half that applies is sent, so switching a server from stdio to remote
        // does not leave the other half of its old definition behind.
        if (remote) {
          draft.url = url.trim();
          draft.command = "";
          draft.args = [];
          draft.headersSet = headers.set;
          draft.headersRemove = [...headers.remove];
        } else {
          draft.command = command.trim();
          draft.args = args
            .split("\n")
            .map((line) => line.trim())
            .filter(Boolean);
          draft.url = "";
        }
      }
      if (await onSubmit(name.trim(), draft)) onCancel();
    },
    [
      invalid, scoped, timeoutSecs, env, patchOnly, transport, auth, remote, url, command, args,
      headers, name, onSubmit, onCancel,
    ],
  );

  return (
    <form className="stg-form" onSubmit={submit}>
      <div className="stg-field">
        <label className="stg-label" htmlFor="stg-server-name">
          Name
        </label>
        <input
          id="stg-server-name"
          className="stg-input"
          value={name}
          readOnly={Boolean(server)}
          placeholder="linear"
          spellCheck={false}
          onChange={(event) => setName(event.target.value)}
        />
        <p className="stg-hint">
          Letters, digits, <code>-</code>, <code>_</code> and <code>.</code> — it becomes a
          key in <code>mcp.json</code> and an argument to the proxy.
        </p>
      </div>

      {patchOnly ? (
        <p className="stg-note">
          {server?.name} is built into opman. What is saved here is a patch on opman's own
          definition — which runners see it, and how long a tool call may take — so its
          launch command is never copied into your config where it would go stale.
        </p>
      ) : (
        <>
          <div className="stg-field">
            <span className="stg-label">Transport</span>
            <div className="stg-segmented" role="group" aria-label="Transport">
              {TRANSPORTS.map((option) => (
                <button
                  key={option}
                  type="button"
                  className={option === transport ? "stg-seg is-active" : "stg-seg"}
                  aria-pressed={option === transport}
                  onClick={() => setTransport(option)}
                >
                  {option}
                </button>
              ))}
            </div>
          </div>

          {remote ? (
            <div className="stg-field">
              <label className="stg-label" htmlFor="stg-server-url">
                URL
              </label>
              <input
                id="stg-server-url"
                className="stg-input"
                value={url}
                placeholder="https://mcp.example.com/sse"
                spellCheck={false}
                onChange={(event) => setUrl(event.target.value)}
              />
            </div>
          ) : (
            <>
              <div className="stg-field">
                <label className="stg-label" htmlFor="stg-server-command">
                  Command
                </label>
                <input
                  id="stg-server-command"
                  className="stg-input"
                  value={command}
                  placeholder="npx"
                  spellCheck={false}
                  onChange={(event) => setCommand(event.target.value)}
                />
              </div>
              <div className="stg-field">
                <label className="stg-label" htmlFor="stg-server-args">
                  Arguments
                </label>
                <textarea
                  id="stg-server-args"
                  className="stg-input stg-textarea"
                  rows={3}
                  value={args}
                  placeholder={"-y\n@scope/package"}
                  spellCheck={false}
                  onChange={(event) => setArgs(event.target.value)}
                />
                <p className="stg-hint">
                  One per line, so an argument may contain spaces. <code>{"${dir}"}</code>{" "}
                  and <code>{"${session}"}</code> are filled in per session.
                </p>
              </div>
            </>
          )}

          <div className="stg-field">
            <span className="stg-label">Credential</span>
            <div className="stg-segmented" role="group" aria-label="Credential">
              {AUTHS.map((option) => (
                <button
                  key={option}
                  type="button"
                  className={option === auth ? "stg-seg is-active" : "stg-seg"}
                  aria-pressed={option === auth}
                  disabled={!remote && option !== "none"}
                  onClick={() => setAuth(option)}
                >
                  {option}
                </button>
              ))}
            </div>
            <p className="stg-hint">{AUTH_NOTE[auth]}</p>
          </div>
        </>
      )}

      <div className="stg-field">
        <span className="stg-label">Runners</span>
        <div className="stg-checks">
          {runners.map((runner) => (
            <label key={runner} className="stg-check">
              <input
                type="checkbox"
                checked={scoped.includes(runner)}
                onChange={() => toggleRunner(runner)}
              />
              {runner}
            </label>
          ))}
        </div>
        <p className="stg-hint">Select none to offer it to every runner.</p>
      </div>

      <div className="stg-field">
        <label className="stg-label" htmlFor="stg-server-timeout">
          Tool timeout
        </label>
        <input
          id="stg-server-timeout"
          className="stg-input stg-input-short"
          value={timeoutSecs}
          placeholder="seconds"
          inputMode="numeric"
          onChange={(event) => setTimeoutSecs(event.target.value)}
        />
        <p className="stg-hint">
          Blank uses each runner's own default. OpenCode gives up at 60 seconds without
          progress, so a long-running tool needs this set.
        </p>
      </div>

      {!patchOnly && (
        <SecretEditor
          label={remote ? "Headers" : "Environment"}
          existing={remote ? server?.headerNames ?? [] : server?.envNames ?? []}
          edits={remote ? headers : env}
          onChange={remote ? setHeaders : setEnv}
          namePlaceholder={remote ? "Authorization" : "API_KEY"}
        />
      )}

      {problem && (
        <p className="stg-error" role="alert">
          {problem}
        </p>
      )}

      <div className="stg-form-actions">
        <button type="submit" className="stg-btn is-primary" disabled={saving}>
          {saving ? "Saving…" : server ? "Save changes" : "Add server"}
        </button>
        <button type="button" className="stg-btn" onClick={onCancel} disabled={saving}>
          Cancel
        </button>
      </div>
    </form>
  );
}
