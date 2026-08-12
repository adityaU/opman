/**
 * The settings page's route, and the history bookkeeping around it.
 *
 * The bug worth pinning down here is the one that shipped: opening settings from the
 * command palette landed the user back in the session they came from. The palette pushes a
 * history entry when it opens, the row pushes `/settings`, and the palette's close then ran
 * `history.back()` — which pops whatever is on top, and by then that was the page.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useModalState } from "../hooks/useModalState";
import {
  openSettings,
  settingsUrl,
  toggleSettings,
  useSettingsRoute,
} from "../settings-page/useSettingsRoute";

/** A history stack real enough to reproduce the ordering the bug depended on. */
interface Entry {
  readonly state: unknown;
  readonly url: string;
}

let stack: Entry[] = [];
let index = 0;

function apply(entry: Entry) {
  const [path, search = ""] = entry.url.split("?");
  window.location.pathname = path || window.location.pathname;
  window.location.search = search ? `?${search}` : "";
}

const pushState = vi.fn((state: unknown, _title: string, url?: string) => {
  const resolved = url ?? window.location.pathname + window.location.search;
  stack = stack.slice(0, index + 1);
  stack.push({ state, url: resolved });
  index = stack.length - 1;
  apply(stack[index]);
});

const replaceState = vi.fn((state: unknown, _title: string, url?: string) => {
  const resolved = url ?? window.location.pathname + window.location.search;
  stack[index] = { state, url: resolved };
  apply(stack[index]);
});

/**
 * Deferred, as the real one is. That is the entire mechanism of the bug this file guards:
 * a modal's close called `back()`, the row then pushed a URL, and the queued pop landed on
 * the page rather than the modal's own entry.
 */
let pendingBack = 0;
const back = vi.fn(() => {
  pendingBack += 1;
});

/** Let every queued `back()` run, as a task-queue turn would. */
function flushHistory() {
  while (pendingBack > 0) {
    pendingBack -= 1;
    if (index === 0) continue;
    index -= 1;
    apply(stack[index]);
    window.dispatchEvent(new PopStateEvent("popstate", { state: stack[index].state }));
  }
}

Object.defineProperty(window, "location", {
  value: { pathname: "/", search: "", reload: vi.fn() },
  writable: true,
});
Object.defineProperty(window, "history", {
  value: {
    get state() {
      return stack[index]?.state ?? null;
    },
    get length() {
      return stack.length;
    },
    pushState,
    replaceState,
    back,
  },
  writable: true,
});

beforeEach(() => {
  window.location.pathname = "/";
  window.location.search = "?session=ses_1&project=0";
  stack = [{ state: null, url: "/?session=ses_1&project=0" }];
  index = 0;
  pushState.mockClear();
  replaceState.mockClear();
  back.mockClear();
  pendingBack = 0;
});

const here = () => window.location.pathname + window.location.search;

describe("settingsUrl", () => {
  it("leaves the default section out of the URL", () => {
    expect(settingsUrl()).toBe("/settings");
    expect(settingsUrl("appearance")).toBe("/settings");
    expect(settingsUrl("mcp")).toBe("/settings?section=mcp");
  });
});

describe("useSettingsRoute", () => {
  it("is inactive on the chat path", () => {
    const { result } = renderHook(() => useSettingsRoute());
    expect(result.current.isSettingsView).toBe(false);
    expect(result.current.section).toBe("appearance");
  });

  it("reads the section from the URL", () => {
    window.location.pathname = "/settings";
    window.location.search = "?section=skills";
    const { result } = renderHook(() => useSettingsRoute());
    expect(result.current.isSettingsView).toBe(true);
    expect(result.current.section).toBe("skills");
  });

  it("reads the editor section from the URL", () => {
    window.location.pathname = "/settings";
    window.location.search = "?section=editor";
    const { result } = renderHook(() => useSettingsRoute());
    expect(result.current.section).toBe("editor");
  });

  it("falls back to a valid section rather than rendering nothing", () => {
    window.location.pathname = "/settings";
    window.location.search = "?section=nonsense";
    const { result } = renderHook(() => useSettingsRoute());
    expect(result.current.isSettingsView).toBe(true);
    expect(result.current.section).toBe("appearance");
  });

  it("recomputes on a programmatic navigation", () => {
    const { result } = renderHook(() => useSettingsRoute());
    act(() => openSettings("mcp"));
    expect(result.current.isSettingsView).toBe(true);
    expect(result.current.section).toBe("mcp");
  });

  // One back gesture should return to the conversation, not walk back through
  // every section the user looked at.
  it("replaces rather than pushes when switching section", () => {
    const { result } = renderHook(() => useSettingsRoute());
    act(() => openSettings());
    pushState.mockClear();
    act(() => result.current.openSection("skills"));
    expect(pushState).not.toHaveBeenCalled();
    expect(replaceState).toHaveBeenCalled();
    expect(here()).toBe("/settings?section=skills");
  });
});

describe("toggleSettings", () => {
  it("opens the section when it is not the one on screen", () => {
    toggleSettings("mcp");
    expect(here()).toBe("/settings?section=mcp");
  });

  it("leaves settings when the same section is already showing", () => {
    openSettings("mcp");
    toggleSettings("mcp");
    flushHistory(); // toggling out goes *back*, which the browser defers
    expect(here()).toBe("/?session=ses_1&project=0");
  });

  it("switches section rather than leaving when a different one is showing", () => {
    openSettings("mcp");
    toggleSettings("skills");
    expect(here()).toBe("/settings?section=skills");
  });
});

describe("leaving a modal for a page", () => {
  // What the command palette's settings rows do: close without touching history, then
  // navigate. Closing the ordinary way queues a `back()` that outlives the push.
  it("reaches the page and stays there", () => {
    const { result } = renderHook(() => useModalState());
    act(() => result.current.open("commandPalette"));

    act(() => {
      result.current.closeSilent("commandPalette");
      openSettings("mcp");
    });
    act(() => flushHistory());

    expect(here()).toBe("/settings?section=mcp");
    expect(back).not.toHaveBeenCalled();
    expect(result.current.modals.commandPalette).toBe(false);
  });

  // The modal's entry is a throwaway, so the page replaces it rather than stacking on it.
  // Otherwise returning to the conversation costs two back presses.
  it("returns to the conversation in one back press", () => {
    const { result } = renderHook(() => useModalState());
    act(() => result.current.open("commandPalette"));
    act(() => {
      result.current.closeSilent("commandPalette");
      openSettings("mcp");
    });
    act(() => flushHistory());

    act(() => {
      window.history.back();
      flushHistory();
    });
    expect(here()).toBe("/?session=ses_1&project=0");
  });

  // The pop is asynchronous, so a close that queues one cannot be followed by a push.
  it("does not unwind a page pushed after the pop was queued", () => {
    const { result } = renderHook(() => useModalState());
    act(() => result.current.open("commandPalette"));

    act(() => {
      result.current.close("commandPalette");
      openSettings("mcp");
    });
    act(() => flushHistory());

    expect(here()).toBe("/settings?section=mcp");
  });

  it("still unwinds its own entry when nothing navigated", () => {
    const { result } = renderHook(() => useModalState());
    act(() => result.current.open("watcher"));
    act(() => {
      result.current.close("watcher");
      flushHistory();
    });

    expect(back).toHaveBeenCalledTimes(1);
    expect(here()).toBe("/?session=ses_1&project=0");
    expect(result.current.modals.watcher).toBe(false);
  });
});
