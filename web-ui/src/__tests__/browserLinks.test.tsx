import { render, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useBrowserLinks } from "../browser-panel/useBrowserLinks";

afterEach(cleanup);

function Harness({ open }: { open: (project: string, url: string) => boolean }) {
  useBrowserLinks("/repo", open);
  return <><a href="https://example.com/docs"><span>Docs</span></a>
    <a href="https://example.com/file" download>Download</a>
    <a href="#section">Section</a>
    <div className="bwp-urlbar"><a href="https://example.com/">External</a></div></>;
}

describe("links opening in the browser pane", () => {
  it("opens nested link contents and prevents outside navigation", () => {
    const open = vi.fn(() => true);
    const { getByText } = render(<Harness open={open} />);
    const event = new MouseEvent("click", { bubbles: true, cancelable: true });
    getByText("Docs").dispatchEvent(event);
    expect(open).toHaveBeenCalledWith("/repo", "https://example.com/docs");
    expect(event.defaultPrevented).toBe(true);
  });

  it("keeps modified clicks, downloads, anchors and external controls native", () => {
    const open = vi.fn(() => true);
    const { getByText } = render(<Harness open={open} />);
    for (const name of ["Download", "Section", "External"]) {
      getByText(name).dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    }
    getByText("Docs").dispatchEvent(new MouseEvent("click", { bubbles: true, ctrlKey: true }));
    expect(open).not.toHaveBeenCalled();
  });

  it("keeps the browser default when no workspace can accept the link", () => {
    const { getByText } = render(<Harness open={() => false} />);
    const event = new MouseEvent("click", { bubbles: true, cancelable: true });
    getByText("Docs").dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });
});
