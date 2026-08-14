import React from "react";
import { ExternalLink, Send } from "lucide-react";
import { str } from "../tcUtils";
import { useAgentSessionOpener } from "./AgentSessionOpenContext";
import { deliveryLabel, displayTarget, messageRole, messageText } from "./model";
import type { AgentManagerAction } from "./action";

/** The small pieces every manager card body is built from. */

export function MetaChip({
  label,
  value,
  icon,
}: {
  label?: string;
  value: string;
  icon?: React.ReactNode;
}) {
  if (!value) return null;
  return (
    <span className="am-meta-chip" title={value}>
      {icon}
      {label && <span className="am-meta-label">{label}</span>}
      <span>{value}</span>
    </span>
  );
}

/**
 * "Open session" — the link that hands this agent's chat to a pane.
 *
 * Renders nothing when the shell cannot place it (no provider, or a session no
 * open project holds), because a link that does nothing is worse than no link.
 */
export function OpenSessionLink({
  sessionId,
  label,
  variant = "chip",
}: {
  sessionId: string;
  label: string;
  variant?: "chip" | "button";
}) {
  const opener = useAgentSessionOpener();
  if (!sessionId || !opener?.canOpen(sessionId)) return null;
  return (
    <button
      type="button"
      className={variant === "button" ? "am-open-btn" : "am-open-chip"}
      onClick={() => opener.open(sessionId, label)}
      title={`Open ${label} — choose a pane, a split, or a new window`}
    >
      <ExternalLink size={variant === "button" ? 12 : 10} />
      <span>Open session</span>
    </button>
  );
}

export function AgentTranscript({ messages }: { messages: readonly unknown[] }) {
  if (messages.length === 0) return null;
  return (
    <div className="am-transcript">
      {messages.slice(0, 8).map((message, index) => {
        const text = messageText(message);
        if (!text) return null;
        const role = messageRole(message);
        return (
          <div className={`am-transcript-row am-transcript-${role}`} key={`${role}-${index}`}>
            <span className="am-transcript-role">{role}</span>
            <span className="am-transcript-text">{text}</span>
          </div>
        );
      })}
    </div>
  );
}

/** The one-line gist shown in the collapsed head, per action. */
export function Summary({
  action,
  input,
  output,
}: {
  action: AgentManagerAction;
  input: Record<string, unknown>;
  output: Record<string, unknown>;
}) {
  const target = displayTarget(input);

  if (action === "send") {
    const delivery = deliveryLabel(str(output.delivery) || str(input.delivery));
    return (
      <span className="am-card-summary">
        {target}
        <span className={`am-delivery am-delivery-${delivery.replace(" ", "-")}`}>{delivery}</span>
      </span>
    );
  }
  if (action === "progress" || action === "abort" || action === "wait") {
    return <span className="am-card-summary">{target}</span>;
  }
  if (action === "list") {
    const count = typeof output.count === "number" ? output.count : null;
    return <span className="am-card-summary">{count === null ? "" : `${count} sessions`}</span>;
  }
  if (action === "options") {
    const runner = str(output.runner) || str(input.runner) || "default runner";
    const filter = str(input.filter);
    return (
      <span className="am-card-summary">
        {runner}
        {filter && <span className="am-card-model">{filter}</span>}
      </span>
    );
  }

  const runner = str(output.runner) || str(input.runner);
  const model = str(input.model);
  return (
    <span className="am-card-summary">
      {runner || "default runner"}
      {model && <span className="am-card-model">{model}</span>}
    </span>
  );
}

/** The addressed-agent chip, shown where the target is not already the subject. */
export function TargetChip({ input }: { input: Record<string, unknown> }) {
  return <MetaChip value={displayTarget(input)} icon={<Send size={10} />} />;
}
