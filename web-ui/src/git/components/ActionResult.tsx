/**
 * The refusal display.
 *
 * The defect this panel was rebuilt around was a git command failing and the
 * UI saying nothing at all. So a refusal gets a headline in plain language, the
 * server's recovery hint, and git's own words verbatim — the last of these
 * matters because the hint cannot anticipate every remote's error text.
 */

import { useState } from "react";
import {
  AlertTriangle,
  ArrowDownToLine,
  ChevronDown,
  ChevronRight,
  FileWarning,
  GitMerge,
  KeyRound,
  Lock,
  SearchX,
  X,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import type { GitAction, GitFailure } from "../types";

export interface ActionResultProps {
  result: GitAction;
  onDismiss: () => void;
}

interface Copy {
  Icon: LucideIcon;
  headline: string;
  /** Used when the server sends no hint of its own. */
  fallbackHint: string;
}

const COPY: Record<GitFailure, Copy> = {
  auth_required: {
    Icon: KeyRound,
    headline: "This remote needs credentials",
    fallbackHint: "Git could not authenticate. Check your SSH key or credential helper, then try again.",
  },
  dirty_tree: {
    Icon: FileWarning,
    headline: "You have uncommitted changes in the way",
    fallbackHint: "Commit, stash, or discard the listed changes, then run this again.",
  },
  conflict: {
    Icon: GitMerge,
    headline: "Git stopped on conflicting changes",
    fallbackHint: "Resolve the conflicted files and stage them, then continue the operation.",
  },
  rejected: {
    Icon: ArrowDownToLine,
    headline: "The remote rejected this push",
    fallbackHint: "The remote has commits you do not. Pull first, then push again.",
  },
  locked: {
    Icon: Lock,
    headline: "The repository is locked",
    fallbackHint: "Another git process is running. Wait for it to finish, or remove a stale index.lock.",
  },
  not_found: {
    Icon: SearchX,
    headline: "That reference does not exist",
    fallbackHint: "The branch, commit, or path named here was not found in this repository.",
  },
  failed: {
    Icon: AlertTriangle,
    headline: "Git could not complete this command",
    fallbackHint: "The full output below is what git reported.",
  },
};

/** Beyond this, the output is a wall of text and gets folded away by default. */
const INLINE_LINES = 3;

export function ActionResult({ result, onDismiss }: ActionResultProps) {
  const failure: GitFailure = result.failure ?? "failed";
  const copy = COPY[failure] ?? COPY.failed;
  const message = result.message?.trim() ?? "";
  const lines = message ? message.split("\n").length : 0;
  const long = lines > INLINE_LINES || message.length > 400;
  const [open, setOpen] = useState(false);

  return (
    <section
      className="gitp-result"
      role="alert"
      aria-live="assertive"
      data-failure={failure}
    >
      <copy.Icon className="gitp-icon gitp-result-icon" aria-hidden="true" />

      <div className="gitp-result-body">
        <p className="gitp-result-headline">{copy.headline}</p>
        <p className="gitp-result-hint">{result.hint?.trim() || copy.fallbackHint}</p>

        {message ? (
          long ? (
            <div className="gitp-result-output">
              <button
                type="button"
                className="gitp-disclosure"
                aria-expanded={open}
                onClick={() => setOpen((value) => !value)}
              >
                {open ? (
                  <ChevronDown className="gitp-icon" aria-hidden="true" />
                ) : (
                  <ChevronRight className="gitp-icon" aria-hidden="true" />
                )}
                <span>{open ? "Hide git output" : "Show git output"}</span>
              </button>
              {open ? <pre className="gitp-mono-block">{message}</pre> : null}
            </div>
          ) : (
            <pre className="gitp-mono-block">{message}</pre>
          )
        ) : null}
      </div>

      <button
        type="button"
        className="gitp-icon-btn gitp-result-dismiss"
        aria-label="Dismiss this error"
        title="Dismiss"
        onClick={onDismiss}
      >
        <X className="gitp-icon" aria-hidden="true" />
      </button>
    </section>
  );
}
