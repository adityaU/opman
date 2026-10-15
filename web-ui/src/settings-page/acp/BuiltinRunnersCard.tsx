import React, { useCallback, useEffect, useState } from "react";
import {
  fetchRunnerSettings,
  setDefaultRunner,
  setRunnerEnabled,
  type RunnersView,
} from "../../api/runnerSettings";
import { runnerLabel } from "../../engine-picker/paletteRows";

export interface BuiltinRunnersCardProps {
  readonly onError: (message: string) => void;
}

/**
 * The two runners opman serves itself, and the default runner.
 *
 * Switching one off stops it at once; its sessions stay listed but cannot be continued
 * until it is on again. The default — where a new session goes — moves off a runner that
 * is switched off by itself, so this card only has to offer a choice among what is on.
 */
export function BuiltinRunnersCard({ onError }: BuiltinRunnersCardProps) {
  const [view, setView] = useState<RunnersView | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const run = useCallback(
    async (key: string, change: () => Promise<RunnersView>) => {
      setBusy(key);
      try {
        setView(await change());
      } catch (error) {
        onError(error instanceof Error ? error.message : "Could not change the runner");
      } finally {
        setBusy(null);
      }
    },
    [onError],
  );

  useEffect(() => {
    void run("load", fetchRunnerSettings);
  }, [run]);

  return (
    <section className="stg-card">
      <h3 className="stg-card-title">Built-in runners</h3>
      <p className="stg-card-note">
        Runners opman serves itself. A runner that is off is stopped; its sessions stay listed.
      </p>
      {view === null ? (
        <p className="stg-hint">Loading…</p>
      ) : (
        <>
          <ul className="stg-rows">
            {view.builtin.map((runner) => (
              <li key={runner.id} className="stg-row">
                <div className="stg-row-main">
                  <span className="stg-row-head">
                    <span className="stg-row-name">{runner.name}</span>
                    {runner.primary && <span className="stg-tag">started on</span>}
                    {!runner.installed && <span className="stg-tag">not installed</span>}
                  </span>
                </div>
                <div className="stg-row-actions">
                  <button
                    type="button"
                    className="stg-switch"
                    role="switch"
                    aria-checked={runner.enabled}
                    aria-label={`${runner.enabled ? "Turn off" : "Turn on"} ${runner.name}`}
                    disabled={!runner.installed || busy !== null}
                    onClick={() => void run(runner.id, () => setRunnerEnabled(runner.id, !runner.enabled))}
                  >
                    <span className="stg-switch-track" aria-hidden="true">
                      <span className="stg-switch-knob" />
                    </span>
                    <span className="stg-switch-text">{runner.enabled ? "On" : "Off"}</span>
                  </button>
                </div>
              </li>
            ))}
          </ul>
          <label className="stg-field rnr-default">
            <span className="stg-label">New sessions start on</span>
            <select
              className="stg-input stg-input-short"
              value={view.default}
              disabled={busy !== null || view.choices.length === 0}
              onChange={(event) => {
                const runner = event.target.value;
                void run("default", () => setDefaultRunner(runner));
              }}
            >
              {view.choices.map((choice) => (
                <option key={choice} value={choice}>{runnerLabel(choice)}</option>
              ))}
            </select>
          </label>
        </>
      )}
    </section>
  );
}
