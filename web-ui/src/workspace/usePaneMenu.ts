/**
 * The pane overflow menu: which pane's is open, and what is in it.
 *
 * Pulled out of `DesktopWorkspace` because the menu grew a history section, and
 * with it a shell-name cache and a label resolver — three things that exist only
 * to fill one popover, sitting in a file that is otherwise about assembling the
 * workspace.
 */

import { useCallback, useMemo, useState } from "react";
import { nextRevealSeq } from "./fileOpen";
import { paneMenuItems } from "./paneMenuItems";
import { useShellLabels, type ShellLabels } from "./useShellLabels";
import type { PaneMenuItem } from "./PaneMenu";
import type { PaneContext } from "./WorkspaceRoot";
import type { WorkspaceAction } from "./reducer";
import type { PaneId, PaneNode, WidgetState } from "./types";

export interface PaneMenuDeps {
  readonly panes: readonly PaneNode[];
  readonly dispatch: (action: WorkspaceAction) => void;
  /** The shell's own description of a pane — where a chat session's title comes from. */
  readonly describe: (widget: WidgetState | null) => PaneContext;
}

export interface PaneMenuApi {
  readonly open: { readonly pane: PaneId; readonly anchor: HTMLElement } | null;
  readonly items: readonly PaneMenuItem[];
  readonly show: (pane: PaneId, anchor: HTMLElement) => void;
  readonly close: () => void;
  /** Shared with the widget opener, which offers the same shells. */
  readonly shells: ShellLabels;
}

export function usePaneMenu(deps: PaneMenuDeps): PaneMenuApi {
  const { describe, dispatch, panes } = deps;
  const [open, setOpen] = useState<{ pane: PaneId; anchor: HTMLElement } | null>(null);
  const shells = useShellLabels();

  // Re-read the shells as the menu opens, so a Recent row pointing at a shell
  // names the shell rather than falling back to "Shell".
  const refresh = shells.refresh;
  const show = useCallback(
    (pane: PaneId, anchor: HTMLElement) => {
      refresh();
      setOpen({ pane, anchor });
    },
    [refresh],
  );

  const close = useCallback(() => setOpen(null), []);

  /**
   * What to call a past target.
   *
   * Two of the five kinds need a lookup the widget cannot answer for itself: a
   * chat session's title, which `describe` already resolves for the pane header,
   * and a shell's name, which only the server knows. The rest fall back to
   * `targetLabel` inside `paneMenuItems`.
   */
  const labelFor = useCallback(
    (widget: WidgetState) =>
      widget.kind === "terminal" ? shells.labelOf(widget.ptyId) : describe(widget).subtitle,
    [describe, shells],
  );

  const items = useMemo(() => {
    if (!open) return [];
    const pane = panes.find((candidate) => candidate.id === open.pane);
    if (!pane) return [];
    return paneMenuItems({
      pane: open.pane,
      history: pane.history,
      dispatch,
      total: panes.length,
      nextSeq: nextRevealSeq,
      labelFor,
    });
  }, [dispatch, labelFor, open, panes]);

  return useMemo(
    () => ({ open, items, show, close, shells }),
    [close, items, open, shells, show],
  );
}
