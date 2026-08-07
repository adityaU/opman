import React, { useCallback, useState } from "react";
import { ExternalLink, LogOut } from "lucide-react";
import { finishMcpLogin, logoutMcpServer, startMcpLogin } from "../../api/mcp";

/**
 * The OAuth login control for one server.
 *
 * opman usually runs on a remote box, so the loopback address the authorization server
 * redirects to cannot load in the user's browser. That is not a failure to hide: the tab
 * *will* end on an error page, and the URL in its address bar is the credential. So the
 * flow says so plainly and asks for that URL back.
 */

type Phase =
  | { readonly kind: "idle" }
  | { readonly kind: "starting" }
  | { readonly kind: "awaiting"; readonly authorizeUrl: string; readonly redirectUri: string }
  | { readonly kind: "finishing"; readonly authorizeUrl: string; readonly redirectUri: string };

export interface LoginFlowProps {
  readonly name: string;
  readonly authenticated: boolean;
  readonly onError: (message: string) => void;
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function LoginFlow({ name, authenticated, onError }: LoginFlowProps) {
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const [pasted, setPasted] = useState("");

  const start = useCallback(async () => {
    setPhase({ kind: "starting" });
    try {
      const started = await startMcpLogin(name);
      setPhase({ kind: "awaiting", ...started });
      window.open(started.authorizeUrl, "_blank", "noopener,noreferrer");
    } catch (error) {
      setPhase({ kind: "idle" });
      onError(message(error));
    }
  }, [name, onError]);

  const finish = useCallback(async () => {
    if (phase.kind !== "awaiting") return;
    setPhase({ ...phase, kind: "finishing" });
    try {
      await finishMcpLogin(name, pasted);
      // The list refetches on the broadcast the backend sends, so there is nothing to
      // set here beyond closing the flow.
      setPhase({ kind: "idle" });
      setPasted("");
    } catch (error) {
      setPhase({ ...phase, kind: "awaiting" });
      onError(message(error));
    }
  }, [phase, name, pasted, onError]);

  const signOut = useCallback(async () => {
    try {
      await logoutMcpServer(name);
    } catch (error) {
      onError(message(error));
    }
  }, [name, onError]);

  if (authenticated) {
    return (
      <button type="button" className="stg-btn" onClick={signOut}>
        <LogOut size={13} aria-hidden="true" />
        Sign out
      </button>
    );
  }

  if (phase.kind === "idle" || phase.kind === "starting") {
    return (
      <button
        type="button"
        className="stg-btn is-primary"
        onClick={start}
        disabled={phase.kind === "starting"}
      >
        <ExternalLink size={13} aria-hidden="true" />
        {phase.kind === "starting" ? "Opening…" : "Log in"}
      </button>
    );
  }

  return (
    <div className="stg-login" role="group" aria-label={`Finish ${name} login`}>
      <p className="stg-login-note">
        A tab has opened. Approve access there, then paste the URL it lands on — it will
        look like an error page, because <code>{phase.redirectUri}</code> only resolves on
        the machine opman runs on.
      </p>
      <div className="stg-login-row">
        <input
          className="stg-input"
          type="text"
          value={pasted}
          placeholder={`${phase.redirectUri}?code=…`}
          aria-label="Redirected URL"
          spellCheck={false}
          onChange={(event) => setPasted(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && finish()}
        />
        <button
          type="button"
          className="stg-btn is-primary"
          onClick={finish}
          disabled={phase.kind === "finishing" || pasted.trim().length === 0}
        >
          {phase.kind === "finishing" ? "Verifying…" : "Complete"}
        </button>
        <button
          type="button"
          className="stg-btn"
          onClick={() => setPhase({ kind: "idle" })}
          disabled={phase.kind === "finishing"}
        >
          Cancel
        </button>
      </div>
      <a
        className="stg-login-again"
        href={phase.authorizeUrl}
        target="_blank"
        rel="noopener noreferrer"
      >
        Open the approval page again
      </a>
    </div>
  );
}
