import { useCallback, useEffect, useMemo, useState } from "react";
import { useResizable } from "./useResizable";
import {
  loadSidebarPrefs,
  persistSidebarPrefs,
  SIDEBAR_MAX_WIDTH,
  SIDEBAR_MIN_WIDTH,
} from "../utils/sidebarPrefs";

/**
 * The sidebar, and which of the two surfaces beside it has the user's
 * attention.
 *
 * This is all that is left of what used to be a panel manager. The editor, git
 * and terminal panels it also owned are panes in the desktop workspace now, and
 * a second place holding open/closed flags for them would be a second answer to
 * a question the workspace already answers — the kind that drifts and then
 * disagrees.
 *
 * Showing and width are standing preferences, so they are restored on load the
 * way the workspace restores its panes. Focus is not: it is where you happened
 * to be, and the shell decides that for itself on first paint.
 */

/** The two surfaces that can hold focus outside the workspace's own panes. */
export type ShellSurface = "sidebar" | "chat";

export function useSidebarState(initialOpen: boolean) {
  // Read once, at mount: a stored preference wins, and `initialOpen` is what
  // a first-ever visit gets.
  const [stored] = useState(() => loadSidebarPrefs({ open: initialOpen, width: 280 }));

  const [open, setOpen] = useState(stored.open);
  const [focused, setFocused] = useState<ShellSurface>("chat");

  const resize = useResizable({
    initialSize: stored.width,
    minSize: SIDEBAR_MIN_WIDTH,
    maxSize: SIDEBAR_MAX_WIDTH,
  });

  // Written when the toggle lands and when a drag ends — not on every pointer
  // move, which would be a localStorage write per frame for no extra fidelity.
  const { size, isDragging } = resize;
  useEffect(() => {
    if (isDragging) return;
    persistSidebarPrefs({ open, width: size });
  }, [open, size, isDragging]);

  const toggle = useCallback(() => setOpen((value) => !value), []);
  const focusSidebar = useCallback(() => setFocused("sidebar"), []);
  const focusChat = useCallback(() => setFocused("chat"), []);

  return useMemo(
    () => ({ open, setOpen, toggle, resize, focused, focusSidebar, focusChat }),
    [open, toggle, resize, focused, focusSidebar, focusChat],
  );
}

export type SidebarState = ReturnType<typeof useSidebarState>;
