import React from "react";
import { MessageSquare, Plus, SquareTerminal } from "lucide-react";
import { WIDGET_ICON, WIDGET_LABEL } from "../PaneHeader";
import { projectColorVars, projectInitials } from "../ProjectBadge";
import { WIDGET_KINDS, type WidgetKind } from "../types";
import type { OpenerChoice, OpenerDraft, StepId } from "./steps";

/**
 * What a row *is*, per step.
 *
 * All three steps used to render the same anonymous row — label left, muted
 * hint right — so the breadcrumb was the only thing telling you which question
 * you were answering. Each step now has its own anatomy: a widget's icon, a
 * project's own colour, a session's timestamp column. The shape of the list is
 * the answer to "where am I", and the text is only the confirmation.
 */

/** Why you would pick this widget, in the terms the pane will then be in. */
const KIND_BLURB: Readonly<Record<WidgetKind, string>> = {
  chat: "Talk to an agent",
  files: "Browse and edit files",
  terminal: "A shell in the project root",
  git: "Diff, stage and commit",
  browser: "A page an agent can read and drive",
};

function asKind(value: string | null): WidgetKind | null {
  return value !== null && WIDGET_KINDS.includes(value as WidgetKind) ? (value as WidgetKind) : null;
}

/** Two-line rows carry a sub; the session and shell lists stay dense. */
export function rowClass(step: StepId, choice: OpenerChoice): string {
  if (step === "kind") return "wsp-opener-row is-kind";
  if (step === "project") return "wsp-opener-row is-project";
  if (choice.value === null) return "wsp-opener-row is-new";
  return `wsp-opener-row ${step === "shell" ? "is-shell" : "is-session"}`;
}

export const RowBody: React.FC<{ readonly step: StepId; readonly choice: OpenerChoice }> =
  function RowBody({ step, choice }) {
    if (step === "kind") {
      const kind = asKind(choice.value);
      const Icon = kind ? WIDGET_ICON[kind] : null;
      return (
        <>
          <span className="wsp-opener-tile" aria-hidden="true">
            {Icon && <Icon size={15} />}
          </span>
          <span className="wsp-opener-text">
            <span className="wsp-opener-label">{choice.label}</span>
            <span className="wsp-opener-sub">{kind ? KIND_BLURB[kind] : choice.hint}</span>
          </span>
        </>
      );
    }

    if (step === "project") {
      return (
        <>
          <span
            className="wsp-opener-chip"
            style={projectColorVars(choice.value ?? choice.label)}
            aria-hidden="true"
          >
            {projectInitials(choice.label)}
          </span>
          <span className="wsp-opener-text">
            <span className="wsp-opener-label">{choice.label}</span>
            {choice.hint && <span className="wsp-opener-sub is-path">{choice.hint}</span>}
          </span>
        </>
      );
    }

    // "New session" / "New shell" answers a different question than the rows
    // below it, so it gets the accent tile and a sub line; the rest do not.
    if (choice.value === null) {
      return (
        <>
          <span className="wsp-opener-tile is-new" aria-hidden="true">
            <Plus size={15} />
          </span>
          <span className="wsp-opener-text">
            <span className="wsp-opener-label">{choice.label}</span>
            {choice.hint && <span className="wsp-opener-sub">{choice.hint}</span>}
          </span>
        </>
      );
    }

    // A shell's hint is what it is *doing*, so it reads as a state rather than
    // as a timestamp — that is the whole reason to pick one shell over another.
    const Glyph = step === "shell" ? SquareTerminal : MessageSquare;
    return (
      <>
        <Glyph className="wsp-opener-glyph" size={13} aria-hidden="true" />
        <span className="wsp-opener-label">{choice.label}</span>
        {choice.hint && (
          <span className={`wsp-opener-when${choice.busy ? " is-running" : ""}`}>
            {choice.hint}
          </span>
        )}
      </>
    );
  };

/**
 * What has been answered so far, each crumb wearing the mark of the row it
 * came from — the widget's icon, the project's hue. Re-reading the words is
 * then optional, which is the point of showing them at all.
 */
export const Crumbs: React.FC<{ readonly draft: OpenerDraft }> = function Crumbs({ draft }) {
  if (draft.kind === null) return null;
  const Icon = WIDGET_ICON[draft.kind];

  return (
    <div className="wsp-opener-crumbs">
      <span className="wsp-opener-crumb">
        <Icon size={11} aria-hidden="true" />
        <span className="wsp-opener-crumb-text">{WIDGET_LABEL[draft.kind]}</span>
      </span>
      {draft.projectPath !== null && (
        <span className="wsp-opener-crumb" style={projectColorVars(draft.projectPath)}>
          <span className="wsp-opener-crumb-dot" aria-hidden="true" />
          <span className="wsp-opener-crumb-text">{basename(draft.projectPath)}</span>
        </span>
      )}
    </div>
  );
};

function basename(path: string): string {
  const parts = path.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}
