import { useCallback, useMemo, useState } from "react";
import { useResizable } from "./useResizable";

/**
 * The sidebar, and which of the two surfaces beside it has the user's
 * attention.
 *
 * This is all that is left of what used to be a panel manager. The editor, git
 * and terminal panels it also owned are panes in the desktop workspace now, and
 * a second place holding open/closed flags for them would be a second answer to
 * a question the workspace already answers — the kind that drifts and then
 * disagrees.
 */

/** The two surfaces that can hold focus outside the workspace's own panes. */
export type ShellSurface = "sidebar" | "chat";

export function useSidebarState(initialOpen: boolean) {
  const [open, setOpen] = useState(initialOpen);
  const [focused, setFocused] = useState<ShellSurface>("chat");

  const resize = useResizable({ initialSize: 280, minSize: 200, maxSize: 500 });

  const toggle = useCallback(() => setOpen((value) => !value), []);
  const focusSidebar = useCallback(() => setFocused("sidebar"), []);
  const focusChat = useCallback(() => setFocused("chat"), []);

  return useMemo(
    () => ({ open, setOpen, toggle, resize, focused, focusSidebar, focusChat }),
    [open, toggle, resize, focused, focusSidebar, focusChat],
  );
}

export type SidebarState = ReturnType<typeof useSidebarState>;
