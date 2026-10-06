import React, { useCallback, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Server, X } from "lucide-react";
import { SERVER_ID } from "../api/base";
import { addServer, removeServer, updateServer, type ServerInfo } from "../api/servers";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { refreshServers } from "../hooks/useServers";
import { switchToServer } from "./status";

/**
 * Add, edit or remove a remote opman server.
 *
 * Home signs in to the remote with these credentials and keeps them; they are written
 * here and never read back, so editing shows the password (and username) empty and an
 * empty field means "keep what is stored". Validation is the backend's: it logs in before
 * saving and answers with a reason, which is shown verbatim.
 *
 * Portalled to `body` so no transformed or blurred ancestor in the sidebar can become
 * its containing block.
 */

export type ServerDialogTarget =
  | { readonly kind: "new" }
  | { readonly kind: "edit"; readonly server: ServerInfo };

export interface ServerDialogProps {
  readonly target: ServerDialogTarget;
  readonly onClose: () => void;
}

interface Draft {
  readonly name: string;
  readonly url: string;
  readonly username: string;
  readonly password: string;
}

function initialDraft(target: ServerDialogTarget): Draft {
  if (target.kind === "new") return { name: "", url: "", username: "", password: "" };
  return { name: target.server.name, url: target.server.url ?? "", username: "", password: "" };
}

function optional(value: string): string | undefined {
  return value.trim() ? value : undefined;
}

async function save(target: ServerDialogTarget, draft: Draft): Promise<void> {
  const name = draft.name.trim();
  const url = draft.url.trim();
  if (target.kind === "new") {
    await addServer({
      name,
      url,
      username: optional(draft.username.trim()),
      password: optional(draft.password),
    });
    return;
  }
  const { server } = target;
  await updateServer(server.id, {
    name: name !== server.name ? name : undefined,
    url: url !== (server.url ?? "") ? url : undefined,
    username: optional(draft.username.trim()),
    password: optional(draft.password),
  });
}

export function ServerDialog({ target, onClose }: ServerDialogProps) {
  const [draft, setDraft] = useState<Draft>(() => initialDraft(target));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [confirmingRemove, setConfirmingRemove] = useState(false);
  const surfaceRef = useRef<HTMLDivElement>(null);
  const editing = target.kind === "edit" ? target.server : null;

  useEscape(onClose);
  useFocusTrap(surfaceRef);

  const field = (key: keyof Draft) => (event: React.ChangeEvent<HTMLInputElement>) => {
    const value = event.target.value;
    setDraft((current) => ({ ...current, [key]: value }));
  };

  const submit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (busy) return;
      if (!draft.name.trim() || !draft.url.trim()) {
        setError("A server needs a name and a URL.");
        return;
      }
      setBusy(true);
      setError("");
      try {
        await save(target, draft);
        await refreshServers();
        onClose();
      } catch (err) {
        setError(err instanceof Error ? err.message : "Could not save the server");
      } finally {
        setBusy(false);
      }
    },
    [busy, draft, target, onClose],
  );

  const remove = useCallback(async () => {
    if (!editing || busy) return;
    setBusy(true);
    setError("");
    try {
      await removeServer(editing.id);
      // This page talks to the server that just went away; nothing on it works now.
      if (editing.id === SERVER_ID) {
        switchToServer("home");
        return;
      }
      await refreshServers();
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not remove the server");
      setConfirmingRemove(false);
    } finally {
      setBusy(false);
    }
  }, [editing, busy, onClose]);

  const title = editing ? `Edit ${editing.name}` : "Add server";

  return createPortal(
    <div className="srv-dialog-backdrop modal-backdrop" role="presentation" onClick={onClose}>
      <div
        ref={surfaceRef}
        className="srv-dialog modal-dialog-surface"
        role="dialog"
        aria-modal="true"
        aria-labelledby="srv-dialog-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="srv-dialog-header">
          <Server size={15} aria-hidden="true" />
          <h3 id="srv-dialog-title">{title}</h3>
          <button type="button" className="srv-dialog-close" onClick={onClose} aria-label="Close">
            <X size={15} />
          </button>
        </div>

        <form className="srv-dialog-body" onSubmit={submit} noValidate>
          <p className="srv-dialog-note">
            Another machine running opman. This server signs in to it and relays it, so its
            projects open here. The password is kept on this server and never sent back.
          </p>
          <div className="srv-dialog-grid">
            <label className="stg-field">
              <span className="stg-label">Name</span>
              <input
                className="stg-input"
                value={draft.name}
                onChange={field("name")}
                placeholder="Dev box"
                autoFocus
                required
              />
            </label>
            <label className="stg-field">
              <span className="stg-label">URL</span>
              <input
                className="stg-input"
                type="url"
                value={draft.url}
                onChange={field("url")}
                placeholder="https://devbox.example.com:8080"
                spellCheck={false}
                required
              />
            </label>
            <label className="stg-field">
              <span className="stg-label">Username</span>
              <input
                className="stg-input"
                value={draft.username}
                onChange={field("username")}
                placeholder={editing ? "Unchanged" : "Optional"}
                autoComplete="off"
                spellCheck={false}
              />
            </label>
            <label className="stg-field">
              <span className="stg-label">Password</span>
              <input
                className="stg-input"
                type="password"
                value={draft.password}
                onChange={field("password")}
                placeholder={editing ? "Unchanged" : "Optional"}
                autoComplete="new-password"
              />
            </label>
          </div>

          {error && (
            <p className="srv-dialog-error" role="alert">
              {error}
            </p>
          )}

          {confirmingRemove && editing ? (
            <div className="srv-dialog-footer is-confirm">
              <span className="srv-dialog-confirm-text">
                Remove <strong>{editing.name}</strong>? Panes showing its projects stop working.
              </span>
              <button type="button" className="stg-btn" onClick={() => setConfirmingRemove(false)}>
                Keep
              </button>
              <button type="button" className="stg-btn is-danger" onClick={remove} disabled={busy}>
                Remove
              </button>
            </div>
          ) : (
            <div className="srv-dialog-footer">
              {editing && (
                <button
                  type="button"
                  className="stg-btn is-danger srv-dialog-remove"
                  onClick={() => setConfirmingRemove(true)}
                  disabled={busy}
                >
                  Remove…
                </button>
              )}
              <button type="button" className="stg-btn" onClick={onClose}>
                Cancel
              </button>
              <button type="submit" className="stg-btn is-primary" disabled={busy}>
                {busy ? "Connecting…" : editing ? "Save" : "Add server"}
              </button>
            </div>
          )}
        </form>
      </div>
    </div>,
    document.body,
  );
}
