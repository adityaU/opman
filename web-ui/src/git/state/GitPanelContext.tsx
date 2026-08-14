/**
 * The bridge between keybinding commands and the views that can carry them out.
 *
 * A few commands — focus the commit message, open the branch picker — can only
 * be performed by the component that owns that widget. Rather than lifting a
 * composer's internals into the shell, a view registers the handlers it can
 * service and the shell dispatches to whatever is currently registered. A
 * command with no registered handler is a no-op, which is the right behaviour
 * when the relevant view is not on screen.
 */

import { createContext, useCallback, useContext, useMemo, useRef } from "react";
import type { ReactNode } from "react";

import type { GitSelection } from "./useGitSelection";

/** Handlers a view can offer to the command layer. */
export interface GitViewHandlers {
  focusCommitMessage?: () => void;
  commit?: () => void;
  generateCommitMessage?: () => void;
  sendToReview?: () => void;
  openBranchPicker?: () => void;
  openRepoPicker?: () => void;
  /** Open a file's diff, switching to the changes view if needed. */
  openDiff?: (path: string, staged: boolean) => void;
  /** Leave the current sub-view, e.g. close an open commit detail. */
  goBack?: () => void;
}

export interface GitPanelBridge {
  selection: GitSelection;
  /** Register handlers for as long as the calling component is mounted. */
  register: (handlers: GitViewHandlers) => () => void;
  /** Invoke a registered handler, or do nothing when none is registered. */
  invoke: <K extends keyof GitViewHandlers>(
    key: K,
    ...args: Parameters<NonNullable<GitViewHandlers[K]>>
  ) => void;
}

const GitPanelContext = createContext<GitPanelBridge | null>(null);

/**
 * Build the bridge. The shell calls this so it can dispatch commands, then
 * publishes the same value to the views through [`GitPanelProvider`].
 */
export function useGitBridge(selection: GitSelection): GitPanelBridge {
  // A set rather than a single slot: two views can be mounted at once, and the
  // most recently registered one wins for any key it provides.
  const registry = useRef<GitViewHandlers[]>([]);

  const register = useCallback((handlers: GitViewHandlers) => {
    registry.current = [...registry.current, handlers];
    return () => {
      registry.current = registry.current.filter((entry) => entry !== handlers);
    };
  }, []);

  const invoke = useCallback(
    <K extends keyof GitViewHandlers>(
      key: K,
      ...args: Parameters<NonNullable<GitViewHandlers[K]>>
    ) => {
      for (let index = registry.current.length - 1; index >= 0; index -= 1) {
        const handler = registry.current[index][key];
        if (handler) {
          (handler as (...rest: typeof args) => void)(...args);
          return;
        }
      }
    },
    [],
  );

  return useMemo<GitPanelBridge>(
    () => ({ selection, register, invoke }),
    [selection, register, invoke],
  );
}

export function GitPanelProvider({
  value,
  children,
}: {
  value: GitPanelBridge;
  children: ReactNode;
}) {
  return <GitPanelContext.Provider value={value}>{children}</GitPanelContext.Provider>;
}

/** Null outside the panel, so a component can be rendered standalone in a test. */
export function useGitPanelBridge(): GitPanelBridge | null {
  return useContext(GitPanelContext);
}
