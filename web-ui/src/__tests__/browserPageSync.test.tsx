import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ScreencastSurface } from "../browser-panel/ScreencastSurface";

const stream = vi.hoisted(() => ({ source: null as (EventTarget & { close: () => void }) | null }));
vi.mock("../api/browser", () => ({
  createBrowserFrameSSE: () => {
    stream.source = Object.assign(new EventTarget(), { close: vi.fn() });
    return stream.source;
  },
  browserKey: vi.fn(async () => {}), browserMouse: vi.fn(async () => {}),
  browserScroll: vi.fn(async () => {}), browserTextInput: vi.fn(async () => {}),
}));

afterEach(cleanup);

describe("native browser navigation metadata", () => {
  it("updates the URL and title from navigation events without polling", () => {
    const onPageChanged = vi.fn();
    const view = render(<ScreencastSurface paneId="proj:/repo" focused={false} scale={1}
      onPageChanged={onPageChanged} />);
    const page = { url: "https://example.com/next", title: "Next page" };
    act(() => stream.source?.dispatchEvent(new MessageEvent("page", { data: JSON.stringify(page) })));
    expect(onPageChanged).toHaveBeenCalledWith(page);
    view.unmount();
    expect(stream.source?.close).toHaveBeenCalledOnce();
  });

  it("ignores broken metadata while the native frame stream stays open", () => {
    const onPageChanged = vi.fn();
    render(<ScreencastSurface paneId="proj:/repo" focused={false} scale={1}
      onPageChanged={onPageChanged} />);
    act(() => stream.source?.dispatchEvent(new MessageEvent("page", { data: "{" })));
    expect(onPageChanged).not.toHaveBeenCalled();
    expect(stream.source?.close).not.toHaveBeenCalled();
  });
});
