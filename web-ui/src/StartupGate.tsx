import React from "react";
import { Check, Circle, LoaderCircle } from "lucide-react";
import { OpmanMark } from "./OpmanMark";
import type { AppState } from "./api";
import type { SSEConnectionStatus } from "./hooks/sse/types";

interface StartupGateProps {
  appState: AppState | null;
  connectionStatus: SSEConnectionStatus;
  initialConnectionsReady: boolean;
  activeSessionId: string | null;
  isLoadingMessages: boolean;
  providersLoading: boolean;
}

interface StartupStep {
  label: string;
  detail: string;
  done: boolean;
}

export function StartupGate({
  appState,
  connectionStatus,
  initialConnectionsReady,
  activeSessionId,
  isLoadingMessages,
  providersLoading,
}: StartupGateProps) {
  const stateReady = appState !== null;
  const sessionsReady = stateReady && appState.startup_ready !== false;
  const liveReady = initialConnectionsReady;
  const workspaceReady = !activeSessionId || !isLoadingMessages;
  const liveDetail = connectionStatus === "disconnected"
    ? "Retrying the real-time event streams"
    : "Opening real-time event streams";
  const steps: StartupStep[] = [
    { label: "Authenticate", detail: "Secure access established", done: true },
    { label: "Load workspace", detail: "Reading projects and preferences", done: stateReady },
    { label: "Hydrate sessions", detail: "Waiting for the session index", done: sessionsReady },
    { label: "Connect live updates", detail: liveDetail, done: liveReady },
    { label: "Prepare tools", detail: "Loading providers and the active session", done: !providersLoading && workspaceReady },
  ];
  const activeStep = steps.find((step) => !step.done) ?? steps[steps.length - 1];
  const completed = steps.filter((step) => step.done).length;

  return (
    <main className="startup-gate" aria-live="polite">
      <section className="startup-card" aria-labelledby="startup-title">
        <div className="startup-mark" aria-hidden="true"><OpmanMark size={24} /></div>
        <h1 id="startup-title">Preparing your workspace</h1>
        <p className="startup-detail">{activeStep.detail}</p>
        <div className="startup-progress" role="progressbar" aria-valuemin={0} aria-valuemax={steps.length} aria-valuenow={completed}>
          <span style={{ width: `${(completed / steps.length) * 100}%` }} />
        </div>
        <ol className="startup-steps">
          {steps.map((step) => {
            const active = step === activeStep && !step.done;
            return (
              <li key={step.label} className={step.done ? "done" : active ? "active" : "pending"}>
                <span className="startup-step-icon" aria-hidden="true">
                  {step.done ? <Check size={13} /> : active ? <LoaderCircle size={14} /> : <Circle size={11} />}
                </span>
                <span>{step.label}</span>
                {active && <span className="startup-step-state">Loading</span>}
              </li>
            );
          })}
        </ol>
      </section>
    </main>
  );
}
