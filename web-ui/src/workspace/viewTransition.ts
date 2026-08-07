/**
 * Run a workspace mutation inside a view transition when the browser has one.
 *
 * Closing a pane is the case this exists for. React unmounts the closed pane
 * on the same frame the reducer drops it from the tree, so there is no element
 * left to animate out — the usual answer is to keep a ghost mounted at the
 * dead pane's last rect and fade that, which means cloning a subtree that may
 * contain an xterm canvas or a live editor. A view transition gets the same
 * result from a snapshot the compositor already has to take, and it animates
 * the part a ghost cannot: the siblings growing into the space, which is what
 * actually tells you where the pane went.
 *
 * Degrades to calling `mutate` directly, which is exactly today's behaviour,
 * so nothing depends on the API being there.
 */

interface ViewTransitionCapable {
  startViewTransition?: (callback: () => void) => { finished: Promise<void> };
}

/** Set while a transition is running so CSS can suppress the enter animations. */
const ACTIVE_ATTR = "data-wsp-transition";

export function withViewTransition(mutate: () => void): void {
  const doc = document as Document & ViewTransitionCapable;
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  if (reduced || typeof doc.startViewTransition !== "function") {
    mutate();
    return;
  }

  // The keyframe entrances in workspace-motion.css would fire on the new
  // snapshot at the same time the transition cross-fades it, which reads as a
  // double animation. The attribute turns them off for the duration.
  document.documentElement.setAttribute(ACTIVE_ATTR, "");
  const transition = doc.startViewTransition(mutate);
  void transition.finished.finally(() => {
    document.documentElement.removeAttribute(ACTIVE_ATTR);
  });
}
